# Evidence: 2026-08-05 static suite audit

- **Date:** 2026-08-05
- **Scope:** read-only defect audit of every project in the suite except the
  Celestina shell, which has its own record in
  [`../../celestina/docs/evidence/2026-08-05-static-shell-audit.md`](../../celestina/docs/evidence/2026-08-05-static-shell-audit.md)
  and is under active correction. Covered: `magnetita/` with
  `celestina-rs/crates/{magnetita-core,magnetita-net,magnetitad}`, `fluorita/`
  with `crates/{fluorita-core,fluorita-engine,fluorita-qt}`, `grafita/` with
  `crates/grafita-core`, `siderita/` with
  `crates/{siderita-core,siderita-ops,siderita-qt}`, and `celestina-style/`
- **Environment:** static source review only. No compiler, formatter, unit
  test, build, smoke run, deployment or suite process ran at any point.
  Repository tooling only (file reads, `rg`, directory listings)
- **Artifact:** none. The output is this record and the remediation plan below

## Procedure

Five concurrent review passes, one per project, each reading its project's
sources in full — including the shared crates where the real logic lives, since
the thin Qt adapters alone prove nothing — and each instructed to trace every
finding to exact source, to read the surrounding context before asserting a
defect, and to discard false positives. Every critical and high finding was
then re-verified independently against the source before being recorded here.
Findings are anchored to symbol and file; line numbers drift.

## Result

### Critical

**MAG-C1 — Argument injection into `sshfs` gives a paired phone arbitrary
command execution.** `magnetitad/src/mount.rs`, `Mount::open`, builds
`format!("{}@{}:{}", sftp.user, host, sftp.path)` and passes it as the first
positional argument. `magnetita-core/src/sftp.rs`, `read_sftp`, accepts any
string for `user` and `path` straight from the `kdeconnect.sftp` packet body,
and `magnetitad/src/main.rs` mounts every reply automatically on connect. A
`user` of `-oProxyCommand=<cmd> #` makes argv[1] start with `-`; sshfs forwards
unknown `-o` options to `ssh`, which runs `ProxyCommand` through `/bin/sh`.
There is no shell in the suite's own call — `ssh` supplies it. Verified in
source; the `-o` forwarding behaviour is sshfs's documented one and was not
executed. Impact: code execution as the desktop user, reachable by a paired
phone, a compromised app on it, or anyone holding it unlocked.

**FLU-C1 — mpv core destroyed with a live `mpv_render_context`.**
`fluorita-qt/cpp/mpvvideoitem.cpp`, `MpvVideoItem::setHandle`, emits
`contextReleased` immediately when `!window() || !isVisible()`, inferring "no
renderer exists" from visibility. `fluorita/qml/components/PlayerSurface.qml`
hides `MpvVideo` whenever playback is not confirmed — including after a stream
error, while renderer and context are still alive. The signal drives
`player.rs::surface_released` → `stop_worker` → drop of `MpvSession` →
`mpv_destroy`, so the core dies before `mpv_render_context_free`, which
libmpv's render API forbids. The same file's own comment states the ordering
requirement. A second path reaches it without any error state: `visible:` and
`handle:` both derive from `renderHandle` and their re-evaluation order is
unspecified. Impact: abort or use-after-free on close after a failed stream.

**GRA-C1 — "Save as" marks keystrokes typed during the write as saved.**
`grafita-core/src/document.rs`, `apply_save`, correctly compares
`report.revision` against the document revision before clearing dirty state.
The save-as path does not: `worker.rs` carries `revision` in
`Completion::Created`, `session.rs::receive_created` destructures it away with
`..`, and `document.rs::adopt_target` calls `history.mark_saved()`
unconditionally, anchoring the save point to the current undo top. Keystrokes
landing during write plus `fsync` are marked saved but never written; the tab
then closes without a prompt, and via `save_and_close` closes outright. Impact:
silent data loss in an editor whose entire design is loss-free byte handling.

### High

- **MAG-A1** — the handshake has no absolute deadline. `magnetita-net/src/link.rs`
  sets a per-syscall `read_timeout`, and `read_delimited_line` reads byte by
  byte, so a byte every nine seconds holds a connection — and an admission
  `Permit` — for days. Eleven LAN addresses exhaust the 42 global permits and
  the real phone is denied permanently, silently (the `None` branch just
  continues). `payload.rs` already implements the correct `remaining_before`
  pattern; the handshake does not.
- **MAG-A2** — a peer-declared `protocolVersion < 8` skips the encrypted
  identity re-exchange (`link.rs::exchange_identity` returns the pre-TLS
  identity, which on the dial path is an unauthenticated UDP datagram's
  contents), skips the pairing timestamp and clock-drift check
  (`magnetita-core/src/pair.rs`), and drops the timestamp from the
  verification key. A LAN attacker announcing the real phone's `deviceId` with
  version 7 gets its own certificate pinned under that id, after which the
  legitimate phone is refused forever as `CertChanged`.
- **MAG-A3** — `wl-paste`, `wl-copy` and `sshfs` run unbounded on the link pump
  thread (`clipboard.rs` uses `Command::output`/`wait`, `mount.rs` uses
  `wait_with_output`). A phone answering SFTP with an unroutable `ip` blocks
  the link indefinitely: no packets read, no pairing clock, device still
  advertised as connected. `media.rs` already applies the correct deadline plus
  process-group teardown discipline; it was not applied here.
- **SID-A1** — pasting into the folder an entry already lives in is not filtered
  (`controller/fileops.rs::paste`, unlike `drop_uris` which does filter), so the
  destination collides with the source itself; choosing "Replace" makes
  `controller.rs::paste_one` trash the target, which *is* the source, and the
  copy then fails with `SourceMissing`. Verified in source. Recoverable from the
  trash, but "paste here" appears to delete the file.
- **SID-A2** — non-UTF-8 names round-trip lossily across the Qt seam
  (`controller/scan.rs` publishes `to_string_lossy`, every verb rebuilds a
  `PathBuf` from the returned `QString`), so those files list with U+FFFD and
  cannot be opened, renamed, copied or trashed. The core preserves bytes
  scrupulously and has tests for it; the seam discards that work.
- **SID-A3** — `portal.rs` answers `writable: true` unconditionally, so a
  sandboxed app that asked to read a file receives write access to it.
- **GRA-A1** — "save as" writes to a percent-encoded name:
  `qml/components/DocumentView.qml::saveTo` does `substring(7)` on the URL
  string while the open path decodes properly through `url::local_path`. A
  chosen destination containing `#` or `%` creates a literally misnamed file
  while the UI reports success.
- **GRA-A2** — `session.rs::save()` guards on neither `busy` nor `dirty`, and
  the Ctrl+S shortcut in `Main.qml` guards on nothing, so a double press queues
  a second save snapshotted against the pre-rename identity; `verify_target`
  then reports `ChangedUnderneath` and pins a false, permanent "another program
  changed this file" banner that nothing clears.
- **GRA-A3** — cancelling the save-as dialog leaves `close_after_save` armed
  (no `onRejected` handler, and `cancel_close` does not reset it): a later
  ordinary save closes the tab by itself, and an abandoned quit sweep leaves
  `quitCursor` set, accumulating zombie tabs.
- **GRA-A4** — `session.rs::receive` applies the staleness filter before
  checking `pending_classify`, so a classify answer superseded by a later open
  is dropped and its `Classified` event never fires. The embedded host's file
  activation dies silently; the code comment describes exactly the case this
  ordering breaks.
- **FLU-A1** — `player.rs::open()` gates only on `render_handle != 0`, so an
  activation during an in-flight close (handle already 0) tears down the
  session with the context possibly unreleased, and the later
  `surface_released` kills the *new* session's worker.
- **FLU-A2** — `FluoritaLibrary::close()` joins from the GUI thread a worker
  whose cancellation token is only consulted in `watch_library`; during scan
  (`poll` with a 180 s timeout, no `cancel_current`) and `learn_tags` (up to 500
  probes × 15 s) it is ignored. Adding or removing a folder mid-scan freezes
  the interface for minutes.
- **FLU-A3** — a failed `mpv_render_context_create` emits nothing, so `Start`
  never arrives and the file is never loaded: video stays "abriendo" forever,
  with no sound, no error and no timeout.
- **FLU-A4** — `PlayerRust` has no `Drop`, and `Main.qml::onClosing` accepts the
  close without waiting for `surface_released`, so core destruction races scene
  graph teardown at exit: intermittent crash on quit with video playing.

### Medium and below

Recorded in full in the per-project sections of the remediation plan below.
The notable clusters: Siderita's portal backend (no overwrite confirmation on
save, `SaveFiles` ignores its `files` option, trash purge addressed by
positional index), Siderita's scan aborting a whole listing when one entry
vanishes mid-scan and surfacing that error even on quiet watcher refreshes,
symlinked directories not navigable, Magnetita's locks held across disk I/O and
its private key written world-readable before the `chmod`, Fluorita's MPRIS
never emitting `PropertiesChanged`, Grafita's per-keystroke O(n) work against a
declared 64 MiB ceiling, and Magnetita rendering peer-controlled strings as
`Text.AutoText` rich text.

### Verified sound (recorded so it is not re-audited)

- **Magnetita:** TLS signatures are genuinely verified (only the *identity* is
  accepted without a CA, which is KDE Connect's TOFU design, not
  `accept_invalid_certs`); path traversal on file receipt is closed
  (`safe_filename` plus `hard_link` publication that cannot clobber); payload
  size, port range and certificate fingerprint are all validated before any
  network read; no `unwrap`/`expect`/indexing on remote data anywhere; all
  channels are bounded; lock ordering is consistent with no cycle.
- **Siderita:** copy uses `create_new`/O_EXCL throughout, so no silent clobber
  even under race; trash follows the freedesktop reservation order with O_EXCL
  on the `.trashinfo`, suffixed collisions and rollback on every failure path;
  cancelled operations roll back the partial destination and keep the source;
  selection and verbs use stable dev+inode+name tokens, not model indices; no
  `sh -c` anywhere.
- **Grafita:** byte round-trip is exact, BOM preserved, no `from_utf8_lossy` on
  any path that writes to disk; all byte offsets validate `is_char_boundary`;
  UTF-16 conversions clamp rather than panic; disk-full leaves the original
  intact; the directory fsync degrades to `Durability::Reduced` instead of
  lying.
- **Fluorita:** no SQL at all (atomic percent-encoded TSV with hostile-file
  budgets); the thumbnail cache keys on the URI's MD5, so no traversal and no
  practical collision; mpv's baseline is hardened (`config=no`, `ytdl=no`,
  `load-unsafe-playlists=no`) and the locale gotcha is handled before type
  registration.
- **celestina-style:** icon catalogue is consistent in all four directions
  (`available`, aliases, `.qrc`, CMake, files on disk); the QML tests assert
  substantive behaviour; no binding loops or binding-killing imperative
  assignments.

## Remediation plan

Ordered by exposure, not by project. Each item names its own verification, and
none of it can be closed while the live-validation hold stands — these are
source corrections whose gates need real-session evidence.

### Stage 1 — reachable code execution and data loss

1. **MAG-C1.** Validate `user` and `path` at the decode boundary in
   `magnetita-core/src/sftp.rs`: allowlist `[A-Za-z0-9._-]+` for the user,
   require an absolute path free of control bytes, and reject any value
   beginning with `-`. Ignore `info.ip` and always mount against the
   TLS-authenticated `link_host` (this also closes MAG-M6). Verify with unit
   tests over hostile packet bodies.
2. **GRA-C1.** Split `adopt_target` into "adopt file" and "mark saved", and
   have `receive_created` compare the carried revision against the document's,
   taking the existing `SaveApplication::StillDirty` path when they differ.
   Same for the close decision. Verify with a core test that edits between
   request and completion.
3. **FLU-C1 with FLU-A1 and FLU-A4.** Replace the visibility inference in
   `setHandle` with an explicit renderer-alive flag set in `createContext` and
   cleared in `releaseContext`; defer `open()` while a close is in flight; add
   `Drop for PlayerRust` that stops and joins, and accept the window close only
   from `surface_released`. These three share one ordering invariant and should
   land together.
4. **SID-A1.** Detect `target == source` by dev+inode in `begin_paste` and
   force `KeepBoth`, matching what `drop_uris` already does by path.

### Stage 2 — network exposure and protocol binding

5. **MAG-A2 with MAG-M2 and MAG-M5.** Floor the protocol at version 8, never
   treat a UDP announcement's identity as the peer's, accept only port 1716 and
   local subnets when dialling, and do not publish a registry entry before the
   trust check.
6. **MAG-A1.** Give the whole handshake an absolute `Instant` deadline checked
   per read iteration, reusing `payload.rs`'s pattern, and log permit
   exhaustion so denial is diagnosable.
7. **SID-A3 with SID-M1 and SID-M2.** Answer `writable` only when the request
   asked for it, confirm overwrite in the save picker, and implement
   `SaveFiles` against its `files` option. One pass over the portal backend.
8. **MAG-M7.** Create the private key with `mode(0o600)` and `create_new`, the
   directory 0700, and write it through `atomic_file::replace` like the rest of
   the suite's state.

### Stage 3 — daily robustness

9. **MAG-A3 and MAG-M1.** Apply `media.rs`'s bounded-subprocess discipline to
   the clipboard and the mount, and move file I/O out from under the
   `Revocations`/device-registry locks.
10. **FLU-A2 and FLU-A3.** Propagate cancellation into scan and tag learning,
    and report a failed render-context creation as an error state.
11. **GRA-A2, GRA-A3, GRA-A4.** Guard `save()` on busy/dirty, handle the save
    dialog's `onRejected`, and check `pending_classify` before the staleness
    filter.
12. **SID-M3 and SID-M4.** Guard trash behind the same `op_running` check paste
    uses, and treat a vanished entry mid-scan as a skipped entry rather than a
    failed listing — and never surface a quiet refresh's error as a banner.

### Stage 4 — correctness debt with a design decision attached

13. **SID-A2 and FLU-M1.** Both projects lose non-UTF-8 names at the Qt seam.
    The fix is the same in both: publish a stable token alongside the display
    string and have the verbs resolve the real `PathBuf` in Rust. This is an
    architecture decision, not a patch, and should be one shared plan.
14. **GRA-A1 and GRA-M2.** Route save-as through `url::local_path`, canonicalise
    the full destination so a symlink is followed rather than replaced, and
    fsync the directory before claiming durability.
15. **GRA-M4 and GRA-M6.** Record replace-all as one change so the undo limit
    cannot split it, and make the per-keystroke path incremental — or lower the
    declared 64 MiB ceiling to what the current architecture actually serves.
16. **STY-M1 and STY-M2.** Derive the scrollbar handle offset from the same
    distances the drag uses, and make the Phosphor sync script write to its work
    directory and `mv` into place instead of truncating the source file before
    the first download.

### Deferred by decision, not by oversight

Stage 4 item 13 — the non-UTF-8 Qt seam shared by Siderita (SID-A2) and
Fluorita (FLU-M1) — was deliberately left untouched when the rest of this plan
was implemented on 2026-08-05. Both projects lose byte-exact names at the same
boundary and for the same reason, and the fix is the same in both: publish a
stable token beside the display string and resolve the real `PathBuf` in Rust.
Patching either one alone would leave two divergent seams and a half-migrated
contract, which is worse than the current honest limitation. It needs its own
cross-project plan and an architecture decision on where the token lives. The
defect stands as recorded: those files list and cannot be operated on.

Stage 4 item 15's performance half (GRA-M6, per-keystroke O(n) against a
declared 64 MiB ceiling) and STY-B5 (full-document gutter reindex on every
keystroke) are likewise deferred: both are incremental redesigns whose value
has to be measured, not asserted, and neither is a correctness defect.

### Not scheduled

MAG-B6 (any session-bus peer can ask the daemon to send any readable file to
the phone) and SID-B8 / GRA-B6 (unauthenticated session-bus entry points) are
properties of the session bus trust model shared with every comparable desktop
component. They belong in the threat model documentation, not in a correction.

## Limits

Static review only. Three findings are marked speculative by the pass that
raised them and are recorded as such: Fluorita's raw-pointer teardown window
(FLU-M7, depends on Qt internals), Grafita's U+2028 round-trip through
`QTextDocument` (GRA-M7, needs a live check), and Siderita's trash purge race
(SID-M7, real pattern, small practical window). Every other finding cited above
was traced to its source and read in context. No claim here rests on a grep.
