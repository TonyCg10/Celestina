# Evidence: 2026-08-18 compress, extract, and operations that run at once

- **Date:** 2026-08-18
- **Scope:** `SID-A1-B`; plan
  [archive-compression](../plans/archive/2026-08-18-archive-compression.md);
  domain evidence [archive domain](2026-08-18-archive-domain.md)
- **Environment:** Arch-derived Linux, Qt 6.11.1, `cargo` stable. The author ran
  the deployed binary on the live session; everything else is offscreen
- **Artifact:** `siderita/target/release/siderita`, built and verified by
  `scripts/complete-production.sh` and deployed to `~/.local`

## What the application gained

Right-clicking an archive offers to extract it, and any selection offers to
compress it. An encrypted archive stops and asks for its password once, then
resumes the same batch; skipping one does not abandon the others. Writes no
longer take turns: each is a job with its own cancellation, and the surface
shows one ring per job with the action's own icon, opening a callout that names
what it is doing and cancels that job alone.

## Procedure

| Check | Result |
|---|---|
| `cargo test` | 110 tests pass |
| `cargo clippy --all-targets` | no warnings of ours |
| `scripts/qml-tests.sh` | 71 tests pass |
| `scripts/smoke.sh` | binary alive 8 s, no QML errors, no auto-bindings |
| `scripts/verify-production.sh` | verified; artifact sealed |
| `scripts/complete-production.sh` | built, verified and deployed the same bytes |
| Repository guards | language, architecture, style and qmllint contracts pass |

## Result

All of the above pass, and the deployed binary is the one those bytes were
verified as. The author ran both verbs on the live session on their own
archives, which is what turned up everything in the next section.

## What the author found that the tests had not

Five defects reached the author's machine. Each is recorded because each shows a
check that was missing, not merely a mistake:

1. **An extraction hung forever.** The tool's stdout pipe filled and it blocked
   in `write` while we waited for it to exit. Any archive with enough members
   did it; the 7z used for checking was too small. Both pipes are now drained on
   their own threads.
2. **The progress bar sat at zero.** It was fed the archive count, not the
   members, and the value published for a lone archive was `0` rather than
   "unknown" — a rule that had been overwritten by a later rewrite of the same
   function. Two attempts aimed at the animation could not have worked.
3. **The byte count appeared halfway through.** `unrar` finishes a member's line
   only when the member is done, and that archive's first member is 26.7 GB. The
   name is now read from the unfinished line and the member weighed on disk as
   it grows: 0.52 GiB at one second, where nothing at all appeared before.
4. **A hidden staging folder instead of the files.** Correct, and useless to
   watch for an hour. The visible destination is created up front, atomically.
5. **The callout's pointer pointed nowhere and could not be dismissed.** A
   rotated square read as a loose diamond; it is a drawn triangle now, and a
   press outside or `Escape` closes the callout.

The offscreen renders used for checking run the software renderer, which
repaints unconditionally. That is why a still ring passed twice here and failed
there — the checks that replaced those now assert the value the controller
publishes and the position of the drawn shape, neither of which needs a frame.

## Limits

`VAL-SID-07` — the author's own run of both verbs on the live session,
including a RAR without a password, which no archive on this machine can
provide.
