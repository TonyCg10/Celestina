# Evidence: 2026-08-08 a journal that survives the freeze

- **Date:** 2026-08-08
- **Scope:** Celestina `DIAG-1`, unit `DIAG-1-A`
- **Artifact:** the verified and deployed `celestina` 0.8.0 production bundle.
- **Environment:** Linux 7.1.6 (CachyOS), Rust 2021 workspace, Qt 6.9, CMake
  Debug build tree for CTest. Celestina was never started, activated or
  restarted; Noctalia owned the session throughout; no `ddcutil`, DDC bus, real
  Wayland connection, real MPRIS player or live surface was touched.

## What was implemented

A structured, persistent, always-on, bounded diagnostic journal, so that after a
physical freeze and reset the seconds before it can be reconstructed from the
disk rather than from a terminal buffer.

| Piece | Where |
|---|---|
| Vocabulary, levels, fields, redaction, bounds, JSONL line, rotation and the drop policy — all pure | `celestina-shell-core::diagnostics` |
| The sink: bounded queue, writer thread, rotation, private permissions, recovery, critical flush, bounded drain | `celestina-shell-core::journal` |
| The host's mirror of the same contract | `celestina/src/diagnosticjournal.{h,cpp}` |
| Instrumentation | `main.cpp`, `niriclient.cpp`, `shellprovidersclient.cpp`, `niri_adapter.rs`, `provider_adapter/{tools,brightness,main}.rs` |
| Read-only collection | `celestina/scripts/diagnostic-report.sh` |
| Documentation | `celestina/docs/diagnostics.md` |

## Procedure

```sh
bash scripts/check-architecture-contract.sh
```

Exit 0 — `Sealed colour contract: OK (4 colour(s))`, `Contrast contract: OK`,
`QML visual contract: OK`, `Architecture contract: OK`.

```sh
python3 scripts/check-language-contract.py
```

Exit 0 — `Language contract: OK (157 legacy file(s) ratcheted)`; the baseline did
not move.

```sh
cd celestina-rs && cargo fmt --all --check
cd celestina-rs && cargo clippy --locked -p celestina-shell-core --all-targets -- -D warnings
cd celestina-rs && cargo test -p celestina-shell-core
```

Exit 0 — **300 tests**, up from 273. The 27 new ones cover the JSONL line, the
identity and `run_id` correlation across three components, monotonic ordering
when the wall clock steps backwards, redaction, the hostile-secret fixtures, text
bounds, line overflow, the rotation arithmetic, file retirement, all three
branches of the queue's drop policy, the published loss event, real files in
temporary directories, an unwritable directory, a journal with nowhere to write
at all, a torn last line, private permissions, and the bounded shutdown drain.

```sh
cd celestina && cargo fmt --all --check
cd celestina && cargo clippy --all-targets --locked -- -D warnings
cd celestina && cargo test --bins
```

Exit 0 — 20 Niri-adapter tests and **51** provider-adapter tests, up from 45. The
six new ones use fake processes only (`/bin/sh -c …`, a nonexistent path) and
prove: a bounded process recorded from spawn to exit with its code, PID and
duration and with its output measured but not kept; a process killed at its
deadline recorded as a timeout and then as reaped; a cancelled process recorded
as cancelled rather than timed out; a spawn failure that keeps its technical
`ErrorKind` instead of collapsing to "missing"; a launch that records that it
happened and nothing about what was opened; and two DDC operations never
overlapping, with a deliberately nested pair proving the `ddc.overlap` detector
itself works.

```sh
cd celestina && cmake --build build -j8
ctest --test-dir celestina/build --output-on-failure
```

Exit 0 — the host compiles with its journal, and CTest is **17/17**.

The final preactivation review additionally found and corrected three defects:

- both writers claimed a 1.5-second shutdown bound but performed an unbounded
  thread join; they now detach safely after the deadline while retaining
  process-lifetime storage;
- the host recorded Niri-helper lifecycle but not provider-helper generation,
  restart backoff, failure and shutdown, the lifecycle adjacent to automatic
  DDC startup; those events are now critical records;
- the Niri adapter's intentional `process::exit(0)` path bypassed destructors
  and therefore its final journal drain; it now records the host-input closure
  and closes the journal before exiting.

After those corrections, both Rust format and strict Clippy checks passed, the
core suite remained **300/300**, the helper suites remained **20/20** and
**51/51**, the host rebuilt, and CTest passed **17/17**. The tray integration
test requires the private D-Bus socket it creates itself; it failed to start
inside the filesystem sandbox and passed outside it against that private bus.

```sh
celestina/scripts/diagnostic-report.sh --output /tmp/... --boot 0
```

Exit 0 against a fixture journal in a scratch `XDG_STATE_HOME`. It started
nothing, ran no DDC, changed no service, and printed exactly the files it read
and the files it wrote.

## Architecture, and the one boundary that is duplicated

Policy lives once, in `celestina-shell-core`. Both Rust helpers use it directly.

The host does **not**, and that is a declared mirror rather than an oversight.
The host is a separate C++ process that does not link the crate — the helpers
are spawned executables, not a library — and the events the host most needs to
record are exactly the ones that happen when no helper is alive to record them:
a helper failing to start, a helper dying, the backoff between restarts, and the
host's own shutdown. Routing host events through a helper's pipe would lose
precisely those. So each of the three processes writes its own file, all in one
directory, all carrying one `run_id`, and a reader merges them by timestamp.
There is no shared writer, no cross-process lock and no file two processes
append to — which is also what lets the files survive one process being killed.

`clippy::struct_excessive_bools` was not silenced anywhere and no `#[allow]`,
`unsafe`, `unwrap`, `expect` or `panic!` was added to a production path.

## Data deliberately omitted

Clipboard content, notification bodies and titles and action labels, media
titles and artists and albums and URLs, passwords and tokens and secrets and
non-essential SSIDs, window titles, the command lines of applications launched
from `.desktop` files, and external frames or payloads. Where one matters to the
timeline, only its size is recorded.

In Rust this is structural: such a value can only enter through `Redaction`,
which has no constructor that retains the text and no accessor that returns it.

**Content is not hashed**, and that is a decision rather than an omission. A
hash of a short string is not irreversible in practice — a window title, a track
name or an SSID is guessable — so a digest would invite the brute force it
appears to prevent while looking like protection.

`ddcutil detect` output is measured, never kept: it prints EDID, serial numbers
and monitor models, which identify hardware in somebody's room and answer no
question this journal exists to answer. The connector and display number are
kept, because a DDC diagnosis is written in them.

## Production exit

The final preactivation pass ran `scripts/complete-production.sh` outside the
filesystem sandbox because `celestina-tray-watcher` must create its own private
D-Bus socket. The registered chain rebuilt the release bundle, passed all Rust,
QML, style and C++ evidence including CTest **17/17**, ran only the isolated
`--pick-output` smoke under Qt `offscreen` with a nonexistent session bus and
scratch XDG directories, verified the artifact manifest and deployed those
exact bytes to `~/.local`. It did not activate or replace the live shell and it
started neither helper, so it performed no DDC or real MPRIS operation.

## Limits

- **Nothing here was ever run in a real session.** Every journal line in every
  test came from a test fixture. No line in this record was produced by a real
  helper, a real DDC operation, a real compositor or a real freeze.
- **This does not establish causation, and cannot.** It records what this shell
  did. A DDC write immediately before a device loss is a coincidence in the
  record, not a cause; the same record would appear in a session where the fault
  lay entirely in the driver, the firmware, the power supply or the hardware.
- **The last write can still be lost.** Critical events are `fsync`'d, but a
  machine that loses power between the write and the flush loses that line. The
  absence of a closing event does not prove the operation was still running.
- **Silence is not innocence.** An event class nobody instrumented leaves no
  line, and the absence of a line is not evidence that nothing happened.
- **Coverage is deliberately crash-oriented.** Provider-helper lifecycle is
  recorded in full, while media and transient-UI instrumentation is thinner
  than the process, DDC and Niri paths. Media is a passive MPRIS consumer and
  starts no decoder or GPU process; pointer-level UI logging would add volume
  without locating a PCIe loss. The host's own writer has no unit
  test target — the host has no test seam for it today — and is covered by
  review and by compiling into all four host test binaries.
- **The rotation and retirement paths were exercised synthetically**, by
  rotating a sink in a temporary directory, not by filling 4 MiB.

## Result

The journal is implemented, tested, documented, collectable and deployed in
0.8.0, which is also `WSG-1`'s version: the two units reach the repository in
one commit and therefore in one version transition, because their inventories
were both measured against the same parent and the contract admits one
transition per commit. Its delivery shares one atomic worktree batch with the
previously
completed but uncommitted `WSG-1`; their inventories divide whole paths and
explicitly record that no intermediate checkout existed. This preserves the
bytes actually reviewed instead of inventing a historical boundary.

`VAL-DIAG-1` — a real session producing real journals, and a bundle collected
after a real freeze — is the author's and has not been run.
