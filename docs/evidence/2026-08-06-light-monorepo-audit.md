# Evidence: 2026-08-06 light monorepo audit

- **Date:** 2026-08-06
- **Scope:** light read-only audit of the whole monorepo at `5924ad4`, in three
  passes: the Celestina shell after `LVR-3-B` landed; the fifteen commits
  delivered today across `siderita`, `fluorita`, `grafita`, `magnetita` and
  `celestina-style`; and the repository infrastructure — `scripts/`,
  `.githooks/`, the four `build.rs`, both `CMakeLists.txt` and
  `docs/projects.toml` — which had never been audited
- **Environment:** static review. The GPU safety hold was respected in full: no
  Celestina binary, provider, build, test, deployment or activation ran. The
  read-only repository guards and their seven test suites were run, plus
  `cargo test` for the shared crates excluding `celestina-shell-core`, and one
  isolated offscreen Qt probe in scratch space to settle finding C1
- **Artifact:** none. The output is this record and the plan below. Nothing was
  corrected: the author asked for the audit and the plan only

## Procedure

Three concurrent passes, deliberately lighter than the 2026-08-05 sweep, each
told to prefer a few verified findings over many speculative ones. The first
re-read what `LVR-3-B` changed rather than re-auditing the shell. The second
read today's diffs against what they claimed to fix. The third read the guards
that decide whether a change may enter the repository at all. Every high finding
below was then re-verified independently against the source before being
recorded here.

## Result

The headline is uncomfortable and worth stating plainly: **today's delivery
introduced three of the four most serious findings.** The corrections were real,
but two of them broke something adjacent and one made a safety mitigation
ineffective. That is an argument about verification depth, not about the fixes
themselves — and it is exactly what a follow-up audit is for.

### Critical

**C1 — Siderita's thumbnail fix broke every accented filename.**
`siderita/cpp/thumbnailprovider.cpp`, `ThumbnailProvider::requestImageResponse`.
Commit `032860a` changed `QUrl::fromPercentEncoding(id.toUtf8())` to
`QByteArray::fromPercentEncoding(id.toLatin1())`. The byte-level decoder was the
right call; `toLatin1()` was not. Qt does not hand a provider the literal key: it
derives the id with `url.toString(RemoveScheme|RemoveAuthority).mid(1)`, whose
default `PrettyDecoded` formatting **has already decoded every `%XX` that forms
valid UTF-8**. So U+00F1 arrives as one character and `toLatin1()` collapses it
to the single byte `F1`, where the file on disk holds `C3 B1`; anything outside
Latin-1 becomes `?`. The invalid `%FF` this change existed for survives either
way, because Qt cannot decode it.

Verified twice: by reading Qt's id derivation, and by an isolated offscreen Qt
probe that compiled a real `QQuickImageProvider` and printed the bytes for
`ni%C3%B1o.png`, `na%FFme.png` and `mis%20fotos/a.png`.

Failure scenario: every name carrying an accent or a tilde now fails its `stat`,
so `loadThumbnail` returns a null image and the entry falls back to its generic
glyph, and the shared thumbnail cache is neither read nor written for it. On a
Spanish desktop that is the picture folder itself and most of what is in it. The
rare case was fixed by breaking the common one.

Why the tests missed it: `siderita/src/thumbnails.rs` calls the C++ helpers with
raw bytes, which skips the `QUrl` → `id` seam where the defect lives. The test
proves the function, not the path that feeds it.

Fix: `QByteArray::fromPercentEncoding(id.toUtf8())`, correct for both cases, and
a test whose input crosses `QUrl` the way Qt's own call does.

### High

**H1 — The shell's escalation timer can SIGKILL a healthy replacement helper,
mid-`ddcutil`.** `celestina/src/shellprovidersclient.cpp`,
`ShellProvidersClient::helperError`. `LVR-3-B` replaced an immediate `kill()`
with TERM-then-KILL, but the `QTimer::singleShot` lambda captures `this` and
tests `m_process`, and that `QProcess` is reused for the replacement. Nothing
binds the timer to the instance it was armed for. With `gracefulShutdownMs =
3000` and a backoff starting at `initialRestartDelayMs = 250` and doubling, the
replacement starts at 250/500/1000/2000 ms — all inside the window — and takes
the `kill()`. Its first act is `ddcutil detect`, and SIGKILL skips the whole
cancellation chain, abandoning a child on the i2c bus: the precise shape that
preceded both retained GPU losses. Fix: bind the timer to a process generation
counter, or cancel it in `startHelper()` and `helperStopped()`.

**H2 — The 20-second spacing after an unclean exit never engages.** Same file,
`scheduleRestart`. `m_uncleanExit` is set only in `helperStopped`, but
`helperError`'s `NotRunning` branch also calls `scheduleRestart()`, which
returns early when the timer is already active. Whichever handler wins the race
fixes the delay, so `qMax(m_restartDelayMs, abandonedChildLifetimeMs)` is
effectively unreachable for a crash. The mitigation written to remove the DDC
overlap does not remove it. The Qt emission order (`errorOccurred` before
`finished` for `CrashExit`) is the one premise not verifiable under the hold,
but the design flaw stands either way: the decision depends on a race the code
does not control. Fix: decide the delay when the timer fires, or let
`helperStopped` — the only handler that knows `exitStatus` — own the restart.

**H3 — Fluorita's player can be left dead for the rest of the session.**
`fluorita/src/player.rs`, `close`/`run_session`. `run_session` publishes the
render handle asynchronously with no generation guard. The new shortcut in
`close()` — when `render_handle == 0`, call `surface_released()` and return —
runs inside that window, joins the worker and destroys the mpv instance, and the
already-queued closure then lands and sets `render_handle` to a freed
`mpv_handle`. The state that remains, `render_handle != 0` with `worker ==
None`, makes every later activation a silent no-op: `decide_open` routes to
`CloseFirst`, `close()` returns early on `worker.is_none()`, and nothing
replays `pending_open`. Reachable by pressing Escape immediately after
activating a video. A use-after-free is the probable second consequence if the
item is still in the scene graph. Fix: carry a session generation and drop a
stale `set_render_handle`, the pattern `grafita-core` already uses for
`Completion`; and make `close()` resolve `pending_open` instead of returning.

**H4 — Producer markup still renders in two shared controls.** `LVR-3-B` set
`textFormat: Text.PlainText` on the shell's own `Text` items, but notification
*action labels* and tray *menu* labels are not `Text` items — they are
`CelestinaButton` and `GlassMenuItem`, whose `contentItem` in `celestina-style`
has no `textFormat` and therefore defaults to `Text.AutoText`. A notification
whose action is labelled `<img src=http://host/x>` makes the shell process issue
that request on the producer's behalf. This is the same High finding the
2026-08-05 shell audit raised, closed for text fields and left open for labels.
It belongs to `celestina-style`, not to Celestina, because the controls are
shared with Siderita, Fluorita and Grafita.

**H5 — The documentation contract has been red on published `main` for five
commits, and no local gate runs it.** 104 errors across ten committed files:
five 2026-08-05 evidence records that predate the template, `celestina/VALIDATION.md`,
the active shell plan, and two inventories (see H6). `.githooks/pre-commit` runs
only the language contract and the staged-unit check; `commit-msg` runs only
`commit_scope.py`; and none of the seven `verify-production.sh` scripts runs the
documentation contract. Its only enforcement point is
`.github/workflows/contracts.yml`, so delivery proceeded with CI red.

**H6 — A malformed `Pathspec` silently disables the strongest cross-check
against Git, and it is in committed history.** `scripts/check-staged-units.py`
never parses `Pathspec` at all, and `documentation_contract.pathspec_valid`
does not reject embedded whitespace. In
`siderita/docs/inventories/2026-08-04-shared-reading-surface/SID-G7-D.numstat.tsv`
the seven boundaries sit on one line separated by spaces. The string still
starts with `siderita/`, so it passes the prefix scope check; it then matches no
path, `actual_paths` comes back empty, and the check that an inventory omits no
changed path evaporates for that unit.

**This one is mine.** The generator I wrote passes one `Pathspec` line per
argument, but I invoked it as `... $SIDP` with the boundaries in a shell
variable, and this shell is zsh, which — unlike bash — does not word-split an
unquoted expansion. The seven paths arrived as a single argument. A sweep of
every tracked inventory found exactly one affected: `SID-G7-D`. `SID-G7-E` has
a second, unrelated defect from the same session: its ledger `Diffstat` says
`+699/-83` where the real totals are `+703/-83`.

Both inventories are committed and pushed, and the contract states that a
tracked inventory is immutable — never edited, moved, renamed, recalculated or
reused. Remediation therefore needs an author decision rather than a patch, and
the options are set out in the plan.

### Medium

- **`siderita/src/controller/navigation.rs::path_segments`** publishes
  `name\tkey` and QML splits on the first tab, so a directory whose name
  contains a tab makes the key unparseable and the breadcrumb raises a spurious
  error banner. No wrong-path risk — the remainder never starts with `/`, so
  `decode` answers `NotAbsolute`. Publish key-first, or sanitise the name.
- **`siderita/src/pathkey.rs::normalize`** — the recorded debt understates it.
  A legacy raw path containing a literal `%XX` normalises to a string that is
  already a valid key for a *different* path. For favourites and icons the mark
  is forgotten, as recorded; for bookmarks the entry is also a navigation and
  drop target. Mark the migration explicitly instead of inferring it from codec
  idempotence.
- **`siderita/src/controller/mounts.rs::send_to_phone`** is the one verb that
  still hands a lossy path out of the process, to Magnetita's `SendFile`. It is
  the remaining ADR 0008 hole toward another suite process.
- **`celestina/src/niri_adapter.rs::stream_session`** treats the new
  `WriteError::TooLong` as fatal, so an oversized snapshot tears down and
  reconnects the Niri session in a loop, where before the host merely discarded
  the line. Corner reachability (512 workspaces), real error-handling defect.
- **`scripts/check-language-contract.py`** fails *open* on an unresolvable
  `LANGUAGE_COMPARE_REF` — verified: an all-zero or nonexistent ref prints OK
  with rc=0, while the architecture guard errors. CI passes `github.event.before`,
  which is all-zero on branch creation.
- **`scripts/check-architecture-contract.sh`** hard-codes the application list
  in five functions, so a sixth registered application would be skipped in
  silence; and a missing `$app/qml` makes `find` fail without incrementing
  `failures`.
- **`scripts/qmllint-cxxqt.sh`** has no warning ratchet — warnings may grow
  without limit — and takes `$?` from an `xargs` pipeline, so with enough files
  a failure in a non-final batch reports OK.
- **CI history replay is weaker than `.github/workflows/README.md` claims:** the
  scope-only replay drops the imperative and kind rules, merges are skipped, and
  the replay reads the registry from HEAD, so widening `commit_roots`
  retroactively legitimises old commits.

### Low

Recorded in the passes and carried into stage 4 of the plan: Grafita's `save()`
now returns early on a clean document *before* asking for a destination, so
Ctrl+S on a new empty document does nothing at all; `magnetita-net`'s
`complete_tls` retries without sleeping on a non-blocking `Ok`; Fluorita's
`releaseContext` slightly widens the already-recorded `FLU-M7` raw-pointer
window; a cut pasted into its own folder ends silently with no status text;
`TabStrip.qml` and `FolderHeading.qml` still do string surgery on a display
path, which is the pattern ADR 0008 says should leave QML; `production_artifact`
allows an empty verification glob; `celestina/CMakeLists.txt`'s `DEPENDS` list
has drifted from the real source set (harmless today because the target is
`ALL`); and `commit_scope.read_subject` can validate a line that is not the real
subject.

### Verified sound

Worth recording so it is not re-audited: `MAG-C1` is genuinely closed — the
allowlist, the absolute-path rule, the rejection of the whole reply on one bad
entry, and the removal of the peer-supplied `ip` — as are `MAG-A1`/`MAG-A2` and
the bounded-subprocess module. `GRA-C1` is correct: adopting the file and
declaring the document clean are now two answers, and a pending close is
abandoned rather than honoured. `SID-A1`'s dev+inode identity is right, and does
not follow symlinks. `FLU-C1`'s claim counter is correct in its ordering, and
`PlayerSurface.qml` keeps the item in the graph so the renderer can still
release. The shell's frame budgets now line up constant for constant between
`snapshot.rs` and `providerstates.cpp`, both counting UTF-16 units.

On the infrastructure side, the delivery-integrity core resisted every attack
attempted: `check-staged-units.py` hashes index bytes, rejects pre-existing
inventories, demands exactly one host `done` row and one prefix per batch, and
fails closed on every early return found. The HEAD-interprets-INDEX design is
implemented properly, baselines cannot be raised unnoticed, `docs/projects.toml`
is 100% consistent with the tree, there is no `shell=True` anywhere, temporary
files use `mktemp` with traps, and installs are atomic with backup and restore.
The `rerun-if-changed` suspicion was checked against real build output and is a
false alarm: `cxx-qt-build` emits an entry per `.qrc` resource.

## Plan

Ordered by exposure. Nothing here has been implemented.

### Stage 1 — regressions this session introduced

1. **C1.** Restore `toUtf8()` in the thumbnail provider and add a test whose
   input crosses `QUrl` exactly as Qt's call does, so the seam — not just the
   helper — is covered. This is the one item that degrades everyday use today.
2. **H1 with H2.** Bind the escalation timer to a process generation and let the
   handler that knows `exitStatus` own the restart delay. They are one repair:
   both decide what happens to a DDC-owning helper, and the GPU hold is the
   reason to treat them as urgent rather than tidy.
3. **H3.** Give the player session a generation, drop a stale handle
   publication, and make `close()` resolve `pending_open`.

### Stage 2 — the delivery gates

4. **H5.** Run the documentation contract where it can block: `pre-commit`, or
   at minimum every `verify-production.sh` beside the architecture guard. Then
   bring the five 2026-08-05 evidence records and `celestina/VALIDATION.md` up
   to the template. Editing those records is a change to a hardware-incident
   account and to failed-validation attributions, so their wording is the
   author's call, not mine.
5. **H6.** Reject whitespace inside a `Pathspec` in the parser, and have
   `check-staged-units.py` validate `Pathspec` format, prefix scope and the
   `suite:`-only `.` rule before accepting a batch. Then decide what to do about
   the two committed inventories. Immutability forbids editing them, so the
   choices are: leave them with a recorded erratum; or open one administrative
   unit that supersedes them with corrected successors and says why. My
   recommendation is the erratum — the delivered bytes are correct and fully
   described; what is wrong is one boundary line and one arithmetic cell, and
   rewriting history to fix bookkeeping costs more than it returns.
6. **H4.** Set `textFormat: Text.PlainText` on `CelestinaButton`'s and
   `GlassMenuItem`'s content text, in `celestina-style` so all four consumers
   get it.

### Stage 3 — correctness debt

7. Siderita's breadcrumb separator, the `normalize` blind spot, and
   `send_to_phone` — the last of which needs a decision about whether
   Magnetita's D-Bus surface carries keys, the same decision the portal and the
   clipboard already took.
8. The Niri `TooLong` reconnect loop.
9. The guard weaknesses: the language ratchet failing open, the hand-written
   application list, `qmllint`'s missing ratchet and swallowed exit code, and
   either tightening the CI replay or correcting the README that describes it.

### Stage 4 — low findings

Batch them; none is worth a unit of its own. Grafita's `save()` ordering
deserves a decision rather than a patch: doing nothing on Ctrl+S for an untouched
new document may well be intended, but it is unrecorded.

### Not scheduled

The `;` divergence between GLib and Qt in the thumbnail cache key stands: it is
an interoperability fact, not a defect. `FLU-M7`'s raw-pointer window still needs
a `QPointer` or a shared token to close properly, which is a design change.

## Limits

Static review under a hardware-safety hold. Nothing in the shell was compiled or
run, so H1, H2 and the Niri finding are reasoned from source and from Qt's
documented behaviour, not reproduced. H3 is reasoned about queue ordering rather
than observed. C1 is the one finding settled empirically, and only through an
isolated probe outside the repository — the real path through Siderita's own
binary was not exercised. Nothing here has been corrected.
