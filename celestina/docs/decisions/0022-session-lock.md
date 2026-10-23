# ADR 0022: The session lock is three processes, locked on every failure, with the PAM verdict as its only way out

- **Date:** 2026-10-22
- **Status:** proposed
- **Number:** written by the CEL-2-H-a implementation; provisional until
  landing. The design numbers its decisions H-1 to H-18 and left the ADR number
  open ("the next free number when CEL-2-H lands"). At the time of writing
  `origin/main` holds ADR 0001 to 0020 and the CEL-2-I worktree holds a draft
  0021, so this file takes 0022. If another unit lands a number first, the
  landing renumbers this file and its index row, and nothing else changes. One
  file records the eighteen decisions; the author may split it by group.

## Context

CEL-2-H gives the shell its own session lock, so that Noctalia can stop
(parity design §5: H is required before M). [ADR 0014](0014-lock-and-polkit-carry-over.md)
carried the halted shell's ADR 0004 over on five conditions: its own design, a
threat model with a "never" list, a review by another model, a recovery path
that stays installed until the live checks pass, and a failure mode of
"locked out", never "let in". The
[design](../../../docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md)
(§3 to §8, §14) meets them; its decisions are numbered H-1 to H-18 and are
recorded here as they ended after the implementation and the spikes of
[2026-10-22](../evidence/2026-10-22-cel-2-h-spikes.md). The unit is delivered
in two ledger rows (H-18): CEL-2-H-a, the security core, which this record
accompanies, and CEL-2-H-b, the dimmed wallpaper and the clock. The unit's
evidence is [2026-10-22-cel-2-h-a.md](../evidence/2026-10-22-cel-2-h-a.md).

## Decision

**H-1. Three processes.** The shell supervises; `celestina-lock` owns the
`ext-session-lock-v1` object and the only unlock; `celestina-lock-verify`
answers one PAM question. The lock outlives the shell (its own process group,
`EPIPE` ignored); its status travels on a pipe as bounded lines; only the
compositor's `locked`, relayed by the shell's own child, counts as covered. A
per-display pid file serves the author and nothing else: no decision reads it.

**H-2. Cover first, never decode in the lock.** The toolkit commits a black
cover on each lock surface's first `configure` (and on every later one,
including a `configure` that arrives after `locked`). The lock receives
pre-scaled, pre-blurred pixels in a sealed memfd and opens no user file. In
CEL-2-H-a the shell sends only the header of that bundle; the pixels arrive
with CEL-2-H-b over the frozen format.

**H-3. The verifier is Rust with one FFI module.** `pam.rs` is the project's
second documented `unsafe` exception after `egl.rs`; its hand-written
declarations are checked against the installed header and against the C struct
layouts by tests, and every other module forbids `unsafe`.

**H-4. Password only, versioned contract.** Only the first echo-off prompt of
a call is answered; any other prompt makes the verdict `Unsupported`. Other
factors would arrive as protocol 2.

**H-5. Unlock only on `Authenticated` of the current attempt.** The type has
no public constructor outside the verdict table, `unlock.rs` holds the only
consumption of the toolkit's `UnlockCapability`, and every other outcome
returns to the prompt.

**H-6. Delay 2^n up to 30 s.** Wrong passwords and account refusals wait 2, 4,
8, 16, 30, 30 seconds; unavailable outcomes do not count (they say nothing
about the guess) but wait 2 s. The count survives a relaunch of the lock and
resets on unlock.

**H-7. PAM service `celestina-lock`, installed by the author, no fallback.**
While the file is missing the lock still covers and denies, and the shell warns
once at duty-on. The recommended stack includes `system-auth` and excludes
`pam_nologin`. No agent and no script installs it.

**H-8. Suspend only after `locked`, through a typed proof.** `LockedProof` is
minted only by the policy in the `Locked` state and carries a generation that
the logind worker compares right before `Suspend`. Every other sleep is gated
by a `delay` inhibitor held while unlocked; logind's `Lock` signal is not
served and `Unlock` is never served.

**H-9. The prompt follows keyboard focus.** The focused output shows the
prompt; the others show the cover (and, from CEL-2-H-b, the dimmed backdrop)
only. Before any keyboard enter the prompt follows the pointer, and defaults
to the first output by connector name.

**H-10. The lock's clock runs on an absolute realtime timer** that fires on
resume and on clock steps. Delivered by CEL-2-H-b; the bundle already carries
the offset and the flag that hides the clock when the offset is unknown.

**H-11. `duties.lock`, off by default; coexistence rests on the protocol's
single lock.** A refused lock never suspends. With Noctalia running, whichever
locks first holds the session and the other is refused.

**H-12. Recovery.** `swaylock` stays installed and bound until the author has
passed every live row and retires it in a separate change. The shell relaunches
a dead lock (at most three in sixty seconds). The author kills a stuck lock by
its pid from another VT; when `faillock` or `pam_nologin` closes the user's
own paths, root on a VT or over `ssh` is the last resort. The README carries
the procedure.

**H-13. Hardening.** Non-dumpable lock and verifier with a tracer check;
cleared environments with allowlists (no `HOME`, no `XDG_*` but the runtime
directory); binaries pinned by descriptor beside the shell; identity from the
uid; zeroized secret buffers; redacted key events; never `NO_NEW_PRIVS`.

**H-14. ADR 0004 of the halted shell is superseded for this project** by H-1
to H-13 and H-15 to H-18, which keep its boundary (the lock owns the surface,
PAM owns the verdict, failure is locked) and change its mechanisms.

**H-15. The lock paints on the CPU into `wl_shm`,** with one embedded font and
no GPU, fontconfig or icon theme in the process. EGL only if a measured frame
says so; the key-press cost measured in release at 4K and scale 1.5 is p99
0.65 ms, so it does not.

**H-16. The lock's vocabulary lives in `celestina-lock-proto`,** a crate
without dependencies, and the transitive closure of both binaries is pinned by
a test against a checked-in allowlist.

**H-17. Explicit requests that fail are told to the author** as a desktop
notification (bounded, best effort, at most one identical notice per ten
seconds); idle refusals stay in the log. The lock fades to black when nobody
is there.

**H-18. CEL-2-H is delivered as two units** (the author's decision of
2026-10-17): CEL-2-H-a, the security core with the backdrop bundle format
frozen, reviewed by A and B; then CEL-2-H-b, the backdrop and the clock,
reviewed by A, which may not change what H-a froze.

### Amendments to the design (the author's decisions of 2026-10-22)

The spikes contradicted two statements of the design and the author decided on
2026-10-22 how to proceed. These amend H-2, H-9 and the threat model; the design
document is not edited.

- **A1. The cover is a zeroed `wl_shm` XRGB8888 buffer, always.** niri serves
  `wp_single_pixel_buffer_manager_v1` only under `#[cfg(test)]` (spike Q4), so
  the single-pixel path of design §3.1, §4.6 and §4.13 cannot run on this
  compositor. It was deleted in the toolkit, not left dormant, and the lock
  profile binds no single-pixel global. The viewport scales the buffer to the
  configured logical size; at scale 1.5 on 4K the check held with no protocol
  error.
- **A2. The input-method grab and the accessibility keyboard monitor are a
  recorded residual of X2, accepted by the author, not mitigated.** In smithay
  an input-method client's keyboard grab survives the lock's focus change and
  can be taken while locked, and niri's `org.freedesktop.a11y.KeyboardMonitor`
  emits every key with no lock check (spike Q16). Design §4.6.1 "Input", §5
  TM-15 and TM-28 therefore do not hold on this compositor against a same-user
  process. Closing the paths needs a change in niri. The README states the
  limit next to the X2 limit that was already conceded.
- **A3. The fade delay and the scrim are local constants of the lock.**
  `FADE_AFTER` (30 s, in `machine.rs`), `SCRIM` (black at 60 %) and `VEIL`
  (black at 35 %), both in `view.rs`, replace the tokens `lock.idle_fade_after`
  and a lock scrim of design §4.5 and §4.6. `celestina-style/tokens.toml` and
  its generator are a suite change; they take these values when the style
  program decides. The existing `scrim` token (black at 45 %) reaches only
  3.2:1 over pure white, below the 4.5:1 the field's text needs (T-T1).

- **A4. The lock refuses to lock outside the shell or a verified nest.** On
  2026-10-22 an agent ran `celestina-lock --rehearsal` without `env -i`, with
  `WAYLAND_DISPLAY=wayland-1` inherited from the author's desktop session. The
  lock connected to the real niri and locked the real session; it ignores
  `SIGTERM` by design and a rehearsal never unlocks, so the author had to
  restart the machine. Nothing in the design stopped a client of the lock
  profile from connecting to whatever compositor the environment named.
  Decision (round 5): the connection is opened in one place, the guard of the
  lock profile (`celestina_wayland::lock::ConnectPolicy`), before the first
  Wayland request, and `run_lock` takes the policy as a parameter with no
  default. The policies are `HandedInSocket` (`WAYLAND_SOCKET`: a harness's
  socketpair; the environment points at no compositor; **round 6**: the value
  must be a decimal descriptor number of 3 or more (**round 7**: ASCII digits
  only, by the one function `celestina_lock_proto::wayland_fd`, so `+5` is
  refused although the toolkit would read it), and the guard reads the
  `SO_PEERCRED` of that very socket, accepting it only when the peer, of the
  same user, is its own parent (the harness that made the socketpair) or a
  verified nest; the production lock is held to the same rule, since the shell
  never hands it a socket; a bad number or a peer that is not valid exits 15
  with nothing sent, while a well-formed number that names a closed descriptor
  exits 12, `ConnectError::InvalidFd` from the toolkit; **residual (Info,
  round 7):** "peer is the parent" takes for granted that the parent is the
  harness. A parent that is a compositor or a proxy making a socketpair for its
  children (sway/swaybg style, waypipe) would hand the lock to the real
  compositor; niri does not launch clients with `WAYLAND_SOCKET`, so this is
  not a plausible accident), `ShellLaunched` (the
  environment carries `CELESTINA_LOCK_LAUNCHER=shell`, which the shell's
  supervisor adds to its exact allowlist) and `VerifiedNest` (the peer of the
  Wayland socket, read with `SO_PEERCRED`, is a process of the same user whose
  `/proc/<pid>/cmdline` carries the argument `celestina-dev-session`, which
  `dev-session.sh` gives the niri it starts). `--rehearsal`, `unlock_probe`
  and the examples never get `ShellLaunched`, marker or not: they need a
  verified nest, or a handed-in socket whose peer is their own parent or a
  verified nest. Every doubt
  (an unreadable `/proc`, an unknown peer, another user's process) is a
  refusal; a refused launch exits 15 with a message that says how to rehearse
  (`lock-rehearsal.sh`, or a shell attached to a nest), and nothing has been
  written to the socket. The same stream that was checked becomes the
  connection, so there is no gap between check and use. **What it is not:**
  both the marker and a command line can be forged by a process that can run
  programs as the user, who could lock the session through any other client
  anyway (the same-user limit, X2). The guard is a barrier against accidents,
  not against an adversary. It was verified against the nest of
  `dev-session.sh --own-bus --monitor none --once` (accepted: the lock covered
  the nest) and against a listener that is not a nest (exit 15, zero bytes).

Two spike answers refine the design without amending it: a delay inhibitor
requested during a pending sleep is refused by logind with
`OperationInProgress` (Q17), so the sleep worker reads that answer as the
expected result of a race, never as a bus failure; and a blocking `zbus` signal
iterator ends only through `close()` or a hand-polled stream with a shared
channel waker, never by dropping connection clones (Q10).

## Consequences

- A lock with no wallpaper and no clock is a complete lock: black covers, the
  field, the status words, Caps Lock and the fade. That is what CEL-2-H-a
  delivers and what the author validates first.
- Three binaries ship (`celestina`, `celestina-lock`, `celestina-lock-verify`).
  The lock pair is built in an invocation of its own, because feature
  unification inside the workspace switches the toolkit's `gl` feature on for
  the lock and changes the shipped bytes (evidence, Task 7, finding 1).
- The project carries a second documented `unsafe` module and a new dependency
  (`zeroize`), both justified in their manifests.
- Residuals the author must accept, written in the evidence and the README:
  the same-user limit (X2, including A2), the password's transient copies in
  memory (TM-13), the two sleep cases logind and niri can make uncovered (F24
  and F33), and a dead lock replaced by any client of the same user.
- `swaylock` and the VT login also go through `pam_faillock`, so a lockout
  closes the author's usual recovery paths; the last resort is root. The live
  row `VAL-CEL2-LOCK-RECOVERY` checks the author knows which one they have.
- A hand-run `celestina-lock` (or its probes) outside a nest is refused with
  exit 15 (A4). Rehearse only through `lock-rehearsal.sh` or a shell attached
  to a nest; the guard is a barrier against accidents, not against a process
  of the same user.
- Nothing about the lock has run in the author's session, against real PAM or
  on real hardware. The seven `VAL-CEL2-LOCK-*` rows of this unit stay pending;
  the duty ships `off`.

## Alternatives

- **EGL in the lock** (H-15): rejected for the GPU, mesa and fontconfig it pulls
  into a process whose job is to read a password; the measured CPU frame is far
  below the budget.
- **One process:** rejected, because the lock must outlive a crash or restart
  of the shell, and the PAM call and the password must not live in the
  shell's long-lived, bus-facing process.
- **A grace period or an "unlocked recently" shortcut:** refused by the design's
  "never" list (item 3).

## Revisit when

niri offers a way to restrict who may replace a dead lock, or stops delivering
keys to an input-method grab and to the accessibility monitor while locked
(A2, H-12); the author wants a second factor (protocol 2, H-4); a measured
frame of the CPU painter exceeds the budget (H-15); `celestina-style` gains a
lock scrim and an idle-fade token (A3); or the author decides to retire
`swaylock` (H-12).
