# Magnetita status

- **Updated:** 2026-09-28
- **Implementation:** `MAG-D1`, the design of the two applications, is the
  one open checkpoint since 2026-09-13, when the author closed the
  own-protocol program (`MAG-P0` through `MAG-P7`) with `VAL-MAG-11`, `-12`,
  `-14` and `-15` passed and `-13` deferred. Its rows `MAG-D1-D` and
  `MAG-D1-E` carry the 2026-09-26 audit's Magnetita fixes: wire admission,
  identity and bounds, then session liveness, released input, the bounded
  mirror feed and FUSE caches, the daemon's stop, the dependency trims and
  the accessible conversation row. Every earlier plan (`MAG-S1`, `MAG-R1`,
  `MAG-R2`, `MAG-M1`, `MAG-P0` to `MAG-P7`) is archived; see the records
  below.
- **Versioning:** from 2026-09-26 a Magnetita product fix lands as
  `magnetita-bug` and moves the PATCH version. The audit found 37 earlier
  product fixes landed as `maintenance` (`MAG-25`), so the installed version
  under-reports what changed before that date; history is immutable, and the
  waiver is recorded in [the MAG-D1 plan](docs/plans/active/2026-09-13-app-design.md).
- **Author validation:** `VAL-MAG-16` and `VAL-MAG-17` check the audit
  fixes on the real session. `VAL-MAG-01` to `-04` and `-06` to `-10`,
  written for the retired KDE Connect wire, are still pending; no live
  observation of that wire is reused for the own one.

## Current checkout truth

- `CONV-1-E` (suite): files dropped on a device card, or on the device page,
  go to that device through `SendFileUri` on the model's worker, with
  `--send`'s sending and failure notice; a URI that names no local file is
  ignored and reported ([evidence](../docs/evidence/2026-10-09-drag-and-drop.md)).
- `CONV-1-D` (suite): `magnetita --send FILE…`, the desktop entry's `send`
  action, sends to the one connected device through `SendFileUri` before Qt
  starts, or opens a device chooser when several are connected and sends the
  pick on a worker while the window shows the result; a file given without
  `--send` is ignored. The entry gains `MimeType=application/octet-stream;`
  ([evidence](../docs/evidence/2026-10-09-open-with.md)).
- **One wire, the suite's own.** `magnetitad` speaks `magnetita-proto` over
  `magnetita-link`'s QUIC on UDP 1760, with certificates pinned by
  fingerprint; the KDE Connect wire, its discovery, pairing, payload sockets
  and the `sshfs` mount were removed in `MAG-P7-C`. The daemon advertises
  `_magnetita._udp` and never dials: the phone finds it by the QR or the
  advertisement.
- **Pairing and identity.** The app's pairing action arms a two-minute QR
  window; only a verified proof closes it, and wrong proofs refuse their
  address, never the window. A session runs under the id its pinned
  certificate gives, and a hello naming another id is closed. A phone that
  dials again while its earlier session is open supersedes it.
- **Negotiated capabilities.** Both ends advertise every capability they
  use and keep, per session, what both hellos offer; each refuses a send
  or a received message of anything else (`AUD-1-E`). A phone build older
  than `AUD-1-E` offered only battery and find, so against this daemon it
  keeps only those two: the desktop and the APK land together.
- **Sessions.** Each session reads its control stream from one task and
  writes it from one writer with a 10 s send deadline; a phone that stops
  reading is closed at the deadline. `Forget` is a durable barrier: the
  session's tick closes the session and acknowledges it, and the D-Bus call
  waits for that off the bus's executor. Adapters that block (the
  clipboard's `wl-copy`, notification calls) run on their own thread.
  Keys and buttons a phone holds are released when its session ends. On a
  daemon stop, what a session had queued on its control stream reaches the
  phone before the connection closes (written within 1 s, its receipt
  acknowledged within 2 s more); a phone that does not acknowledge in time
  is closed anyway, and a file transfer in flight on its own stream is cut
  and resumes on the next session.
- **The daily set** travels on the own wire: battery, find, the clipboard
  both ways, the phone's notifications with their buttons and replies,
  files both ways with resume, the phone's player on the desktop's card and
  the desktop's players on the phone, contacts, SMS and calls.
- **Commands, trackpad and keyboard.** Registered commands (name, program,
  arguments) live in `commands.json` and reach the phone as ids and names;
  a run is a bounded process group. The phone's trackpad and keyboard drive
  one virtual `uinput` device through a rate governor that releases always
  pass.
- **The phone's files** are a FUSE directory at
  `$XDG_RUNTIME_DIR/magnetita/<device-id>/`, served over the `storage`
  capability from the folder the person shared; `mounted` and `mountPath`
  in `Devices1` are unchanged. Listings and caches are bounded, and a stop
  (`SIGTERM`) unmounts before the daemon exits.
- **The mirror** is a window of the application: `Mirror1`'s `StartLink`
  asks the phone for its screen, the daemon writes the raw HEVC or H.264 to
  a FIFO in its runtime directory through a bounded queue that drops to the
  next key frame when the window falls behind, and the app decodes it with
  the suite's engine (libmpv). The window's input goes back over one bus
  connection, a drag's moves merged. The daemon spawns no `mpv` and no
  `ffmpeg`; the adb worker only turns the phone's screen off, and browses
  for adb only while that is wanted.
- **`org.celestina.Devices1`** is served on zbus's tokio backend; its
  `Changed` and `Event` signals are coalesced on one thread. Siderita
  consumes the mount, device and media contract, the Celestina shell the
  phone and battery state; the application owns pairing, diagnostics and
  settings.
- **The application** orders actions on one owned bounded worker and
  reflects only confirmed snapshots. Its device lists are parallel string
  lists replaced whole on each snapshot (`MAG-22`'s row-level models are
  not done). The Messages page does not poll: it asks the phone once when
  it opens (`RefreshSms`) and re-reads the daemon's cache on `Changed`
  (`AUD-1-E`).

## Planned implementation debt

- Row-level models for the device and conversation lists, and position-only
  media updates kept apart from `Changed` (`MAG-22`; needs a real Qt build).
- The mirror window's link watcher polls `LinkState` every 400 ms over its
  one connection; watching a signal needs the daemon to emit
  `PropertiesChanged` for the link mirror (`MAG-13`).
- A mirror touch-up can still be lost on the daemon side when the phone has
  stopped reading and the session's outbox is full; the session then ends
  at its send deadline, and the phone's own gesture timeout lifts the touch.
- Magnetita Android's STATUS version line and README install step
  (`MAG-24`, under `magnetita-android:`).

## Blockers

No implementation blocker is recorded. The real phone and network are
needed only for the author validation queue.

## Evidence boundary

CP0-CP4, dependency decisions and the earlier real-phone observations are in the
[archived roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the exact app/daemon release bundle passed format, Clippy, client/core/net/daemon
tests, QML lint and isolated smoke; loopback tests were rerun outside the socket-
restricted sandbox and passed. See the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). The daemon
service and installed bytes were not touched. On 2026-08-05 the `MAG-S1`
corrections passed format, Clippy, the three crates' unit tests (207 tests, 0
failures), the workspace check, QML lint and the architecture contract; see the
[hardening evidence](docs/evidence/2026-08-05-network-input-hardening.md). No
build, deployment or service action was taken.

On 2026-08-06 `MAG-S1-B` passed format, Clippy and the unit tests for the
`celestina-rs` workspace with `celestina-shell-core` excluded, which the
author's hardware-safety hold puts out of bounds and which this unit does not
depend on. Nothing was exercised over a real bus: the new method's decode and
its refusals are proven, the delivery of a file to a phone is not. See the
[byte-exact send evidence](docs/evidence/2026-08-06-byte-exact-send-to-phone.md).

## Records

- [Implementation roadmap](ROADMAP.md)
- [Active plan MAG-D1](docs/plans/active/2026-09-13-app-design.md)
- [Archived plan MAG-M1](docs/plans/archive/2026-09-10-app-lifecycle.md)
- [Archived plan MAG-P7](docs/plans/archive/2026-09-10-storage-and-retirement.md)
- [Archived plan MAG-P6](docs/plans/archive/2026-09-09-link-mirror.md)
- [Archived plan MAG-P5](docs/plans/archive/2026-09-09-remote-control.md)
- [Archived plan MAG-P4](docs/plans/archive/2026-09-09-daily-set.md)
- [Archived plan MAG-P3](docs/plans/archive/2026-09-09-android-foundation.md)
- [Archived plan MAG-P2](docs/plans/archive/2026-09-09-link.md)
- [Archived plan MAG-P1](docs/plans/archive/2026-09-09-protocol-core.md)
- [Archived plan MAG-P0](docs/plans/archive/2026-09-07-own-protocol-spikes.md)
- [Archived plan MAG-S1](docs/plans/archive/2026-08-05-network-input-hardening.md)
- [Author validation](VALIDATION.md)
- [Registry entry](../docs/projects.toml)
