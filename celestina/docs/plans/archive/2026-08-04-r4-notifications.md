# R4 — notifications

- **Opened:** 2026-08-04
- **Plan ID:** r4-notifications
- **Closed:** 2026-08-04
- **Successor:** none; the roadmap is idle until an R5 control-centre plan is opened
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** R4
- **Author-validation checkpoint:** `VAL-R4` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

One pure notification state machine can hold every rule the freedesktop
specification imposes — identity, replacement, expiry, actions, capabilities and
caps — so the bus server around it only marshals, and a session that already has
a notification server keeps it untouched.

## Tangible outcome

The shell serves `org.freedesktop.Notifications` when nothing else owns it,
shows a capped toast stack and history with a truthful unread indicator, and
answers Magnetita's real `Notify`/replace/`CloseNotification` flow in an
automated test rather than by inspection.

## Scope

In scope: the pure state machine and its caps; the bounded bus server in the
aggregate provider runtime, claimed only when the name is free; hostile text and
image hints; compact toasts, capped history, do-not-disturb and the panel's
unread indicator; automated producer/consumer proof against Magnetita's exact
call shape.

## Exclusions

Out of scope: taking the name from Noctalia or any running server; persisting
history across sessions; sounds; a notification centre with filters or per-app
policy, which belongs to R5's control surface; and anything that would make the
shell the only possible server on this session.

## Build order

1. Add the pure state machine, identity and caps to `celestina-shell-core`
   with its specification tests.
2. Serve the bus name in the aggregate provider runtime, claimed only when no
   owner exists, with bounded text and hint handling.
3. Add toasts, capped history, do-not-disturb and the unread indicator over the
   shared style contracts.
4. Prove the producer path end to end against Magnetita's `Notify`, replacement
   and close calls.

## Implementation exit

- Identity, replacement, expiry, action, capability and cap tests pass.
- The server refuses to claim a name another process owns, proved without a
  second server running.
- Hostile summary, body, action and image-hint input is bounded rather than
  trusted, with tests naming each bound.
- CMake registration, QML lint and CTest pass.
- Rust format, Clippy and package tests pass; the lockfile changes only by
  the dependency this plan declares.
- The architecture and documentation contracts pass.
- `scripts/complete-production.sh` builds once, verifies those exact bytes and
  updates the on-disk bundle; the live session is never replaced.

R4 implementation closes on this evidence. Real toast appearance, the actual
handover from Noctalia's server, phone notifications arriving over the air and
assistive-technology behaviour remain an independent `VAL-R4` run.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| R4-A | `celestina:` | done | [inventory](../../inventories/2026-08-04-r4-notifications/R4-A.numstat.tsv) | 34 files, +3545/-38 | The pure notification state machine; the bus server that claims the name only when it is free; toasts, the keyboard notification centre, do-not-disturb and the unread indicator; and the two-process proof of Magnetita's flow | [R4 notifications](../../evidence/2026-08-04-r4-notifications.md) | `VAL-R4` |

The four build-order steps closed as one unit, as R3's did: each `done` unit
needs one exclusive inventory *and* one exclusive evidence record, and a single
verification run does not honestly produce four.

## Decisions and rollback

The session already has a notification server: Noctalia's. This shell therefore
claims `org.freedesktop.Notifications` only when the name is free, exactly as
`TrayWatcherService` treats `org.kde.StatusNotifierWatcher`. Nothing in this
plan stops, replaces or races another server, and the rollback for the whole
checkpoint is not starting Celestina's own server.

Magnetita is a real producer already in this suite: it calls `Notify` with an
empty action list, empty hints, the `phone` icon name and a `-1` timeout, and
withdraws through `CloseNotification`
([`magnetitad/src/notify.rs`](../../../../celestina-rs/crates/magnetitad/src/notify.rs)).
That call shape is the compatibility target R4-D proves, not a hypothetical one.

R4-C splits the surfaces by what they are for. A toast never takes focus —
interrupting typing is the one thing a notification must not do — so its
buttons are reachable by pointer there and by keyboard in the notification
centre, which is a focused overlay. Neither surface is the only way to reach an
action.

That split forced the on-screen display out of the corner. R3 put the readout
top-right, which is where notifications belong and where the panel's own unread
indicator points; a volume key pressed while a notification was up would have
painted over it. The readout is now low and centred, and `OverlaySurface` names
the two placements `Corner` and `Readout` rather than one vague `Notification`.

The indicator reads as text rather than a glyph. The suite's icon catalogue is
closed and vendored and has no bell, and inventing one would put non-canonical
artwork into a set that is canonical everywhere else — the same reason
`AudioLevel` shows a number instead of a speaker.

A notification is untrusted input from any application on the session. Summary,
body, action labels, icon names and image hints are bounded and validated in the
pure core before a surface ever sees them; an image hint is a description to be
checked, never bytes to be trusted.
