# Compressing and extracting archives

- **Opened:** 2026-08-18
- **Plan ID:** archive-compression
- **Status:** active
- **Scope:** siderita
- **Implementation checkpoint:** SID-A1
- **Author-validation checkpoint:** VAL-SID-07

## Hypothesis

A pure-Rust archive domain beside `siderita-ops`, holding the same loss-free
guarantees the copy and Trash verbs already hold, will let the file manager
extract and create `.zip` and `.tar.gz` without a single external process, and
without any archive being able to write outside the folder a person chose.

## Tangible outcome

Right-clicking an archive offers the extract verb and right-clicking any
selection offers the compress verb; both run on the shared progress surface with the shared
Cancel button, refuse to overwrite anything, and leave nothing half-written when
they fail or are stopped.

## Scope

- A new pure crate `celestina-rs/crates/siderita-archive`: identify a container
  by its bytes, list it, extract it, create it.
- Read `.zip`, `.tar`, `.tar.gz`; write `.zip` and `.tar.gz`.
- The Qt half in `siderita/src/controller/archive.rs`, reusing the paste
  operation's worker, progress, cancellation and failure reporting.
- The two menu verbs and a compress dialog that asks for a name and a container.
- Registering the new crate in the project manifest and the verification script.

## Exclusions

- Navigating *into* an archive as if it were a folder.
- Encrypted, split or password-protected archives, and `.rar`/`.7z`.
- Single-file `.gz`/`.xz`/`.zst` (a different verb: one compressed file, not a
  container).
- A settings surface for compression level.
- The real-session observation of both verbs, tracked as `VAL-SID-07`.

## Build order

1. The domain crate with its guards and tests, depending only on
   `celestina-core` and `siderita-ops`.
2. The controller adapter and the bridge invokables.
3. The menu items and the dialog, in QML parity across `QML_FILES`.

## Time zones, and how the limitation was removed

A zip stores its date twice, and neither field is obvious:

- The MS-DOS field every reader understands carries **no zone at all**, and each
  tool reads it as local time. Writing UTC into it, as this plan first did, made
  an archive we wrote show up shifted by the local offset in `unzip -l` and in
  any other manager.
- The optional `0x5455` extended-timestamp field carries an exact Unix instant,
  which no zone can shift.

The domain now writes both and, when reading, prefers the exact one. The zone
for the MS-DOS field is not guessed: `siderita_archive::Zone` asks the caller
for the offset in force **at each instant** (so summer time is right for the
date being converted), the application answers it from `localtime_r`'s
`tm_gmtoff`, and `Utc` remains for a caller with no zone information.

## RAR, 7z and encrypted archives

Two containers a desktop keeps meeting are not decodable here. RAR is
proprietary and its only decoder ships under a licence a GPL program may not
link; 7z has no mature pure-Rust reader. They are therefore **delegated** to a
tool the machine already has (`unrar`, `7z`, `7za`, `7zz`), the way the
desktop's own archive managers do — never linked, never a package dependency,
and the verb is simply not offered when no tool is installed.

The boundary that delegation keeps, and the one it cannot:

- Kept: the tool writes into an empty staging folder, its result is checked for
  symlinks that escape it, a failure removes the staging whole, and cancelling
  kills the process. The argument vector is built directly — no shell — with
  `--` closing the option list and stdin on `/dev/null`, so a crafted file name
  is data and a password prompt can never hang the operation.
- Not kept: the member-by-member guard does not run *during* the extraction.
  Progress *is* reported, read from the line each tool prints as it finishes a
  member — the alternative, one step per archive, showed a still bar for the
  length of a 40 GB extraction.

Draining both of the tool's pipes on their own threads is not an optimization:
a tool whose stdout pipe fills up blocks in `write` and never exits, so the
first version deadlocked on any archive with enough members to fill 64 KB of
output — the author's 1.5 GB RAR froze at 391 MB, extracting nothing further and
never finishing.

Encryption is answered by the same verb. A zip is decrypted here (ZipCrypto and
AES, through `zip`'s `aes-crypto`); a RAR or 7z is decrypted by the tool. Both
report the same two domain errors — "needs a password" and "that password is
wrong" — and neither tool distinguishes them, so the difference is drawn from
whether the caller had supplied one. The password lives exactly as long as the
call: it is never stored, never logged, and never written into an error.

The host asks only when the domain says it must: the worker stops, hands the
rest of the batch to the Qt thread, and the dialog resumes it. Skipping one
encrypted archive does not abandon the others in the batch.

## One operation at a time, and why that stopped being true

Every write verb used to claim the application: one progress bar, one Cancel
button, one `op_running` flag, and every other verb refused while it was set. A
paste could not start while a trash ran — and a stuck extraction froze every
write there is.

What replaces it is a register of jobs: each has an id, its own cancellation
token and its own counters, several run at once, and the surface draws one row
per job with its own Cancel. `cancel_op` still exists and now cancels all of
them.

Concurrency is only safe because the domain stopped relying on look-then-rename.
`std::fs::rename` replaces its destination silently, so two writers racing for
one name could destroy each other's result; `siderita_ops` now *reserves* the
destination (`create_new` for a file, `create_dir` for a directory, both atomic)
and renames onto its own reservation, so the loser of a race is told the name is
taken. That closes a window the crate had documented rather than fixed — one
that had been survivable only while a single write ran at a time.

Undo stays single: it reverses the last finished write, so it is offered only
while nothing is running.

## What a long extraction has to look like

Three things the author saw on a 43 GB RAR, and what each of them was:

- **A dotted folder instead of their files.** The extraction wrote into a hidden
  staging directory and promoted it at the end — correct, and useless to watch
  for an hour. It now creates the *visible* destination up front, under the name
  it will keep (taken atomically with `create_dir`, so nothing existing is
  written into and two extractions cannot claim one name), and the files appear
  inside it as they are written. The archive's own single top-level folder, if
  it turns out to have one, is lifted out at the end with two renames.
- **A bar at zero that never moved.** The bar was fed the *archive* count
  (one of one), not the members, which nothing can count in advance for a
  delegated tool. A job with no known fraction now publishes `-1` and the row
  draws a travelling bar instead of a still one; the byte read-out beside it was
  already truthful.
- **No glass.** The panel was a plain tonal surface while every other floating
  element in the folder carries the suite's glass. It is a `GlassCard` sampling
  the folder behind it, like the dialogs.

## The surface the author asked for

A bar spanning the window for an hour is the wrong shape for this work, and the
one it replaced was worse: a stack of rows that took a third of the folder.

What runs now is a dock of rings, one per job, resting over the content: the
action's own icon at the centre, its progress around it, and the detail one
press away in a callout that points at the ring it belongs to — so Cancel
belongs to the job a person is looking at. A job whose end nothing can predict
turns instead of filling, which is the only honest thing a ring can do when the
tool does not say how many members are left.

Two things this cost, both of them real bugs found on the way:

- **Compress and extract had no icons.** `archive-insert` and `archive-extract`
  are freedesktop names that the suite's closed catalogue does not carry, so
  both fell through to the generic file glyph. Two Lucide-shaped SVGs were
  drawn for them and registered in the catalogue, the QRC and the CMake
  resource list; the action a job is doing now travels to QML as a token
  (`JobKind`), never as its Spanish label, so a translated word cannot decide
  which glyph appears.
- **Two layout cycles.** Sizing a container from its content while positioning
  the content from the container is a cycle, and QML answers it by laying
  nothing out — which is why the previous panel's rows drew on top of one
  another and why the callout's button fell off its card. Both are now sized in
  one direction only.

A note on the earlier finding: the stacked rows were reported as a positioner
bug. They were not — a positioner lays out in the polish phase, and the test was
measuring before a frame had been rendered. `waitForRendering` is what the tests
do now.

Three more the author found on the running build, all of them things a test
could have caught and did not:

- **The ring did not turn** — through three attempts, and only the last diagnosis
  was the real one. The ring was never in its turning state at all: the rule that
  publishes `-1` for an unknowable fraction had been overwritten by a later
  rewrite of the same function, so a lone archive published `0` and the ring drew
  a full, motionless dial. Two "fixes" aimed at the animation (a bound transform,
  then a `RotationAnimator`) could not have worked, and the offscreen check that
  passed them ran the software renderer, which repaints everything anyway.
  What runs now takes the turn from the data: each progress report steps the
  arc's `startAngle`, which changes the path itself. It cannot be starved by a
  renderer, it is testable without a frame, and a job that stops reporting
  visibly stops turning.
- **The byte count arrived halfway through.** `unrar` finishes a member's line
  only when the member is done, and that archive's first member is 26.7 GB — so
  nothing at all was reported for the first half hour. The name is now read from
  the *unfinished* line and the member is weighed on disk on every poll, adding
  only its growth. Measured on the author's 43 GB RAR: 0.52 GiB at one second,
  13.2 GiB at twenty-five, all of it inside that first member.
- **The pointer pointed nowhere.** It was a square rotated 45°, which at this
  size reads as a loose diamond rather than as a tip. It is now a real triangle,
  drawn pointing down and overlapping the card by a pixel. The test that missed
  it measured the *property* holding the position; the one that replaces it
  measures the drawn shape against the ring it must sit under.
- **A press outside did not close the callout.** Only the ring toggled it. A
  catcher under the dock, alive only while a callout is open, closes it — and so
  does `Escape`.

## Measuring before extracting

A ring that turns says "something is happening". It cannot say how much is left,
and for a 45 GB archive that is most of what a person wants to know.

So the archive is now weighed first: `measure` reads the index — headers only,
never member data — and answers how many bytes it holds once extracted. A
container this domain reads sums its own index; a RAR or 7z asks its tool for a
listing, which needs the password when the headers are encrypted, and which it
already has by then. On the author's 43 GB RAR the whole pass takes 30 ms.

With the total known, the ring fills, the callout reads "12,4 GiB de 45,3 GiB"
and carries a bar. Without it — an archive whose listing cannot be parsed — the
ring goes back to turning and the read-out to a plain count, which is the same
honest fallback as before.

Movement is interpolated in QML between reports: they arrive at most every 60 ms
and the screen redraws far more often than that, so a `Behavior` carries both
the bar and the arc from one report to the next instead of stepping between
them.

## Implementation exit

Close `SID-A1` when a zip and a tar.gz round trip through the domain in tests, a
member that would escape the destination fails the whole extraction, a cancelled
run leaves the destination untouched, and
`scripts/complete-production.sh` builds, verifies and deploys those exact bytes.
The author's own extraction of a real archive on the live session belongs to
`VAL-SID-07`.

## Change and commit ledger

The work was declared first as eleven fine-grained units (`SID-A1-A` through
`SID-A1-K`), one per session of it. They were consolidated into the two below
before the first inventory commit, because every one of them touched files the
others touched and a unit's Pathspec has to be exact and disjoint. Nothing was
committed under the old ids, and what each of them delivered is described in the
sections above.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-A1-A | `siderita:` | done | [inventory](../../inventories/2026-08-18-archive-compression/SID-A1-A.numstat.tsv) | 23 files, +4079/-29 | The pure archive domain — sniff, list, extract, create, zone-correct dates, encrypted members, RAR and 7z delegated to an installed tool — and the no-replace rename in `siderita-ops` that makes two writers in one folder safe | [evidence](../../evidence/2026-08-18-archive-domain.md) | `None` |
| SID-A1-B | `siderita:` | done | [inventory](../../inventories/2026-08-18-archive-compression/SID-A1-B.numstat.tsv) | 38 files, +3019/-310 | The application: the two menu verbs, the compress and password dialogs, the job register that lets writes run at once, and the dock of rings that reports them | [evidence](../../evidence/2026-08-18-archive-verbs.md) | `VAL-SID-07` |

Like every plan in this repository, this one records intent and grants no
authority.
