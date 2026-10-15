# The diagnostic journal

Celestina writes a structured, persistent, always-on record of what it did. This
document says what is in it, what is deliberately not, how to collect it after a
physical reset, how to read it, how to quiet it, and — most importantly — what
it cannot tell you.

## Why it exists

This machine's AMD Radeon RX 9070 XT (Navi 48) has more than once ended a
session with:

```text
amdgpu 0000:03:00.0: device lost from bus!
```

Celestina was running each time. The most recent occurrence happened with
Celestina inside a **nested Niri session** started by `scripts/dev-session.sh`.
That nest gave the shell its own outputs, workspaces and surfaces — and shared
everything underneath: the same GPU, the same VCN block, the same DDC/I²C buses
and the same session bus. Two consequences follow, and both narrow what may be
assumed:

- the Noctalia → Celestina → Noctalia handover is **not** a necessary condition;
- a live layer-shell surface on the real session is **not** a necessary
  condition either.

**Correlation is not causality.** Celestina has been present at these events.
That is all the evidence establishes. It does not establish that this shell
caused them, and this journal cannot establish it either — see the limits at the end of this document. What the journal fixes is a separate, narrower defect that is
this shell's own: *after the freeze and the reset, nobody could reconstruct what
Celestina had been doing.*

That was true because the record was scattered Qt messages in the host,
`eprintln!` in the helpers, discarded `stderr` in several external processes, an
optional `CELESTINA_PROVIDER_TRACE` that is normally off, no identity tying any
of it together, no guarantee that a terminal-launched run reaches journald, and
no guarantee that a buffer survives a power cut.

## Where it is and what it looks like

```text
$XDG_STATE_HOME/celestina/diagnostics/
```

`$XDG_STATE_HOME` falls back to `~/.local/state`, so in practice:

```bash
ls -l ~/.local/state/celestina/diagnostics/
```

One file per process per invocation, named `<component>-<run_id>.jsonl`:

| Component | Written by |
|---|---|
| `host` | the C++ Qt host |
| `niri-adapter` | the Rust Niri event-stream helper |
| `provider-adapter` | the Rust aggregate provider helper |

Each line is one complete JSON object. Every line carries the same identity
fields, so the three files merge into one ordering:

| Field | Meaning |
|---|---|
| `v` | schema version of the line |
| `t` | wall-clock nanoseconds since the Unix epoch, UTC |
| `mono_ms` | milliseconds since that process started |
| `level` | `trace`, `debug`, `info`, `warn`, `error`, `critical` |
| `component` | which process wrote it |
| `event` | the class of thing that happened, as `area.thing` |
| `run_id` | one invocation of the host and every helper it started |
| `pid` | the writing process |
| `generation` | which incarnation of that component |
| `worker` | the thread or worker, when one applies |

The host generates the `run_id` and exports `CELESTINA_RUN_ID` **before** it
spawns either helper, so all three files of one session share it. A helper
started by hand generates its own, which is why a reader keys on `run_id` rather
than assuming there is one.

Two clocks are recorded because they fail differently. `t` names a moment you
and `journalctl` can both find. `mono_ms` still orders events correctly when the
wall clock steps — an NTP correction landing mid-incident, a resume, a
time-zone change.

## What is recorded

Host and lifecycle: host start and version, argument **classes**, panel or
`--pick-output` mode, the `run_id`, D-Bus name acquired or refused, each
helper's spawn with its PID and generation, helper exit with code and crash
status, the technical reason behind a helper error, restart backoff, and
shutdown.

External processes: program and full argument list for this shell's **own** tool
invocations, child PID, start, exit code, timeout, cancellation, kill, reap,
duration, and the technical reason for a spawn failure — never reduced to "the
tool is missing", because an exhausted process table looks identical from the
outside.

DDC and brightness: `ddcutil detect` start and end, the technical inventory of
displays found (connector and display number), every read and write with its
output, its display number and its duration, the child PID, hotplug-requested
rediscovery, worker start, and an explicit `ddc.overlap` event if two DDC
operations of this shell were ever in flight at once.

Media: this shell **observes** MPRIS; it decodes no media, launches no decoder
and holds no GPU pipeline. The journal places the provider helper and its
external processes in the timeline. It deliberately does not yet trace each
MPRIS signal: the absence of such a line is not evidence that no signal was
received.

Niri and outputs: connection attempts, disconnection with its reason, snapshots
accepted or skipped, action requests by kind with their outcome and duration.

Providers and commands: the provider-helper generation, spawn, exit, error,
restart backoff and shutdown are recorded by the host, while the helper records
its own lifecycle and every external process it owns. Pointer-level menu and
overlay interaction is not recorded; those events are not adjacent to hardware
and would turn a crash journal into a high-volume interaction log.

Everything about a process, a DDC operation, a helper's death, a compositor
action or shutdown is written at `critical`, which means it is `fsync`'d rather
than left in a buffer a power cut would take.

## What is deliberately not recorded

Never, at any level:

- clipboard content;
- notification bodies, titles or action labels;
- media titles, artists, albums or URLs;
- passwords, tokens, secrets, or SSIDs where they are not indispensable;
- window titles;
- the command lines of applications launched from `.desktop` files;
- external frames or payloads.

Where one of these matters to the timeline, only its **size** is recorded, as
`<field>_chars` and `<field>_bytes`. In Rust that is structural: such values can
only enter through a `Redaction`, which has no constructor that keeps the text
and no accessor that returns it. The host's mirror does the same.

**Content is not hashed.** A hash of a short string is not irreversible in
practice — a window title, a track name or an SSID is guessable, so a digest
would invite exactly the brute force it appears to prevent. Sizes and counts
answer the questions a diagnosis actually asks.

Technical identities a diagnosis cannot do without *are* recorded: output names,
DDC bus and display numbers, provider keys, D-Bus names, program names and error
reasons. `ddcutil detect` also prints EDID, serial numbers and monitor models,
and none of that is kept: it identifies hardware in somebody's room and answers
no question this journal exists to answer.

## Limits, size and rotation

- Each line is capped; a line that would exceed the cap becomes a bounded record
  of the overflow rather than disappearing.
- Each file rotates at 4 MiB, and each component keeps 8 files. One component is
  therefore bounded at 32 MiB, and the whole directory at roughly 96 MiB.
- The queue is bounded. When it is full an ordinary event is dropped and
  counted; a critical event displaces the oldest ordinary one instead. Whatever
  was lost is published as a `journal.dropped` event with a count, so a gap in
  the record always says it is a gap.
- Files are `0600` and the directory is `0700`.
- Writing is append-only, on a writer thread, and never on the Qt thread or a
  hardware worker.
- A journal that cannot be written **never** blocks, fails or terminates
  anything. An unwritable directory is retried; a recovered one records how many
  writes it lost.
- Shutdown gives the writer 1.5 seconds to drain. If the filesystem itself is
  unresponsive, the caller continues and the writer retains process-lifetime
  storage until process exit rather than blocking session shutdown forever.

## Collecting it after a physical reset

```bash
celestina/scripts/diagnostic-report.sh
```

Read-only: it starts nothing, activates nothing, changes no service, runs no
DDC and touches no hardware. It defaults to `--boot -1` — the boot *before* this
one — because the reason to run it is that the machine had to be reset.

```bash
celestina/scripts/diagnostic-report.sh --list
celestina/scripts/diagnostic-report.sh --boot 0
celestina/scripts/diagnostic-report.sh --run <run_id>
celestina/scripts/diagnostic-report.sh --output ~/celestina-bundle
```

It writes one bounded directory containing Celestina's own journals, the run
identifiers present, the graphics and bus lines from `journalctl --dmesg`, the
`celestina` lines from the user journal, a README, and `READ-FILES.txt` listing
every source it read. Home paths, MAC addresses and SSIDs are removed from the
`journalctl` output, which is somebody else's text; Celestina's own lines are
already redacted where they are written.

## Reading it, with read-only commands

Everything below only reads.

```bash
cd ~/.local/state/celestina/diagnostics
```

The runs on disk, newest last:

```bash
grep -ho '"run_id":"[^"]*"' *.jsonl | sort -u
```

One run's three files merged into one timeline, if `jq` is available:

```bash
jq -c 'select(.run_id=="RUN") | [.t, .component, .level, .event]' *.jsonl | sort
```

The last thing every process said, which is where a freeze shows itself:

```bash
for f in *.jsonl; do echo "== $f"; tail -3 "$f"; done
```

Every DDC operation, and whether each one finished:

```bash
grep -h '"event":"ddc\.' *.jsonl | tail -50
```

Any moment two DDC operations of this shell overlapped, which should be none:

```bash
grep -c '"event":"ddc.overlap"' *.jsonl
```

External processes that were killed at a deadline:

```bash
grep -h '"event":"process.timeout"' *.jsonl | tail -20
```

Anything the journal had to drop:

```bash
grep -h '"event":"journal.dropped"' *.jsonl
```

Without `jq`, `grep '"level":"critical"'` narrows any file to the events that
were flushed to disk.

## Quieting the mirror without disabling the journal

Every line goes to the file. Warnings and above are *also* mirrored compactly to
stderr, so journald catches them when the way Celestina was launched allows it.

To quiet only that mirror:

```bash
CELESTINA_JOURNAL_MIRROR=0 celestina
```

The file is unaffected. There is deliberately **no** switch that turns the
critical record off: a diagnostic that can be disabled is one that will be
disabled before the failure it exists for.

## Limits — what this cannot tell you

- **It does not establish causation.** It records what this shell did. A DDC
  write immediately before a device loss is a coincidence in the record, not a
  cause, and the same record would appear in a session where the fault lay
  entirely in the driver, the firmware, the power supply or the hardware.
- **The last write can be lost.** A journal is flushed for critical events, but
  a machine that loses power between the write and the flush loses that line.
  The absence of a closing event therefore does not prove the operation was
  still running.
- **A journal that ends is not a fault that started there.** It shows where the
  record stops. Those are different facts.
- **Silence is not innocence.** An event class nobody instrumented leaves no
  line, and the absence of a line is not evidence that nothing happened.
- **Sizes are not content.** By design, this record cannot answer a question
  that requires knowing what a notification said or what was on the clipboard.
  That is the trade, and it is the right way round.
- **journald may hold nothing.** A terminal-launched run reaches journald only
  when something happens to capture it. That is precisely why the file is the
  evidence and the mirror is a convenience.

## A safe controlled run, later

When the author decides to exercise this against a real session, the order that
keeps the evidence usable:

1. Confirm Noctalia is the rollback and is still installed.
2. Collect a clean baseline first: `diagnostic-report.sh --boot 0` before
   starting anything, so there is a "before" bundle.
3. Start the shell the ordinary way. Do not clear the journal directory: the
   previous run's files are bounded and rotate on their own, and deleting them
   removes the comparison.
4. If the machine freezes and has to be reset, run
   `diagnostic-report.sh` **before** starting Celestina again, so the boot
   selector still names the boot that ended.
5. Compare the last lines of the three files by `mono_ms`, not by `t`.
