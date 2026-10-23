# Evidence: CEL-2-H-a, review B (final verdict)

- **Date:** 2026-10-22
- **Scope:** unit CEL-2-H-a (the session lock's security core), worktree
  `celestina-CEL-2-H-a`, branch `unit/celestina/CEL-2-H-a`, revision reviewed:
  `f9ef5db6` («Fix the second round of lock review findings»). The main record
  is [2026-10-22-cel-2-h-a.md](2026-10-22-cel-2-h-a.md).
- **Environment:** the reviewer worked on a clean clone of that revision, not
  in the worktree, with proofs of concept (PoC) of its own.
- **Artifact:** not applicable. This is a faithful summary of the verdict of
  the final pass of review B as it was handed to the implementer; it is **not**
  the reviewer's verbatim report, which this repository does not hold, and it
  adds no measurement of its own. Where the summary has no detail, none is
  invented.

## Procedure

Review B attacked the lock from the outside, on a clean clone, with PoCs
(summarised in the Result table). It did not run on the author's session or on
the real system bus.

## Result

- **Exit:** verdict reached by revision `f9ef5db6`; three findings (B-1, B-2,
  B-3) and five checks that passed.

### Attacks and checks

| Probe | Outcome |
|---|---|
| 1200 random inputs sent to the verifier | none ended in exit 0 (no unlock without a correct answer) |
| Eight signals sent to the lock, with `setsid` (outside the shell's process group) | the lock survived all eight (the eight of F27) |
| A real cut of the connection to the compositor | the lock exited 16 without having printed `unlocked` |
| R-3, R-4, R-2, R-8, R-9c | PASS |
| Fuzz of the backdrop bundle decoder, 3 million inputs | no panic |

### Findings

| Finding | What the reviewer reported |
|---|---|
| **B-1** | in a degenerate coexistence (another client of the same user competing for the lock) the lock is spawned again every 2 s |
| **B-2** | the signals test (`celestina-lock/tests/signals.rs`) no longer exercises the multi-threaded case that finding C1 was about |
| **B-3** | empty directories |

## Limits

- The disposition of B-1, B-2 and B-3 is **not** decided here: round 4 of
  CEL-2-H-a (the section «Round 4» of the main record) did not touch them. They
  stay open for the author and the next round.
- B-3 is recorded exactly as it was handed over, without interpretation.
- This summary does not carry the reviewer's PoC sources or logs; if they are
  needed they come from the reviewer.

## Follow-up

- B-1, B-2, B-3: to be decided by the author; none is a regression of round 4.
- The main record's «Round 4» section lists what review A's fourth pass found
  and what changed.

## Pass over round 5: `bb519283` («Add the connect guard that refuses to lock outside the shell or a verified nest»)

- **Date:** 2026-10-22 (after the incident recorded in the main record).
- **Environment:** a clean clone of `bb519283` with its own target directory.
  No binary of the lock ran bare: every run used `env -i PATH=/usr/bin
  XDG_RUNTIME_DIR=<scratch>` under `timeout -s KILL` against a **fake
  compositor** (a Python listener that never speaks Wayland) in a scratch
  runtime directory, never against the author's session, its bus or `wayland-1`.
  No process was left running.
- **Artifact:** not applicable. As above, this is a faithful summary of the
  reviewer's report as it was handed to the implementer, not its verbatim text.
- **Verdict of the reviewer: «can land»**, with no security finding that blocks
  it. The guard closes the class of failure of the incident (an accidental
  connection to a real compositor).
- **Attempts that were refused with exit 15 or 12 and zero bytes at the fake
  compositor:** a bare `--rehearsal` with a desktop-like display and a
  non-nest compositor (the original incident); a bare non-rehearsal run with no
  marker; the display given as an absolute path, with `..`, or through a
  symlink (the peer check looks at the process really connected, not at the
  path); no runtime directory; `--rehearsal` **with** the shell marker (the
  marker is ignored for a rehearsal); `WAYLAND_SOCKET` set to a non-number or
  to a regular file; `unlock_probe`, `paint_probe` and `lock_cover` run bare.
- **Attempts that were accepted, as they must be:** a verified nest with
  `--rehearsal`, the shell marker without `--rehearsal` (production), and a
  handed-in `WAYLAND_SOCKET` connected by the harness.
- **Checked by reading and by `/proc`, without running the session:** the
  author's real niri runs as `niri --session` with no `celestina-dev-session` in
  its command line, and only `dev-session.sh` injects that name; the marker
  `CELESTINA_LOCK_LAUNCHER=shell` is set only on the child's environment by the
  supervisor and never exported; the production lock is never refused by the
  guard; a refusal of the guard ends the shell in `Idle` without a relaunch and
  retakes the inhibitor.
- **Finding (Info, does not block):** `rustix` 1.1.5 reads `SO_PEERCRED` into a
  type whose pid cannot be zero, so a peer outside this pid namespace is a
  possible niche undefined behaviour inside the dependency, and the `pid <= 0`
  branch of the nest judge cannot be reached through the real call. It fails
  closed, it is unreachable on the author's machine, and round 6 records it as a
  documented residual.
- **Disagreement with review A:** review A's sixth pass asked to validate the
  peer of a handed-in `WAYLAND_SOCKET` (round 6 does); review B judged that
  variable not to be an accidental path, since only a deliberate `setenv` sets
  it. The implementation follows review A, the more cautious reading.
