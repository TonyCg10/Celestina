# Magnetita status

- **Updated:** 2026-09-10
- **Implementation:** `MAG-P7`, storage over the own wire and the
  retirement of the second path, is the active checkpoint since 2026-09-10,
  paired with `magnetita-android`'s `AND-5`; the storage capability, the
  FUSE mount and the removal of the KDE Connect wire are done, and
  `MAG-P7-D` (the `adb` mirror path) waits on `VAL-MAG-14`. `MAG-P6`, the
  mirror as a capability of the link, is implemented and archived.
  `MAG-P5`, commands, trackpad and keyboard, met its exit the same day and
  is archived; `VAL-MAG-13` carries the author's hand. `MAG-P4`, the
  daily set, met its exit the same day and is archived; `VAL-MAG-12`
  carries the author's daily use. `MAG-P3`, the
  Android application foundation, met its exit on 2026-09-09 and is
  archived, paired with `AND-1`. `MAG-P2`, the link, is closed (`4011e53`, `95a4cc8`):
  `magnetita-link` carries `magnetita-proto` over QUIC with pinned
  certificates, `magnetitad` hosted the own wire next to KDE Connect on one
  runtime thread with `StartPairing` on `Devices1`, and `magnetita-peer`
  drives it from a shell. Deployed on 2026-09-09 and exercised end to
  end by the peer against the live daemon; closed, the author having ruled
  that the only other host is the phone of `MAG-P3`. `MAG-S1`'s pending production exit was carried by the same
  deployment. `MAG-P1` is complete (`07e70c4`, `a071e39`):
  `magnetita-proto` holds the envelope, hello, pairing and the catalog of
  twelve capabilities, 63 tests, with [the wire document](docs/protocol.md). `MAG-P0`'s spikes
  are measured, closed and committed (`ad26b3e`). `MAG-S1` (hostile network input) and
  `MAG-R1` (the one-button wireless mirror) are delivered, committed and
  deployed; `MAG-S1`'s plan is archived with its canonical production exit
  still pending as a deployment action. `MAG-R2` (the mirror without
  discovery) is committed but **not deployed**. `MAG-M1` remains planned and
  unimplemented
- **Author validation:** the original 1.0 daily set passed on the real phone,
  before the 2026-07-29 hardening. **Every one of `VAL-MAG-01` through
  `VAL-MAG-04` and `VAL-MAG-06` through `VAL-MAG-09` is still pending**, so no
  corrected path has an author pass against the phone. This, not the code, is
  Magnetita's largest open risk

## Current checkout truth

- The daemon speaks one wire, the suite's own: the KDE Connect link,
  discovery, pairing v8, payload sockets and the `sshfs` mount are gone
  (`MAG-P7-C`). The phone's files arrive over the `storage` capability
  from the document tree the person shared and are a FUSE directory at
  the runtime path Siderita browses, `mounted` and `mountPath` unchanged
  in `Devices1` (`MAG-P7-A`, `-B`).
- The mirror runs over the link: `Mirror1`'s `StartLink` asks the phone
  for its screen, the raw HEVC arrives on a bulk stream of a fixed id and
  plays in an `mpv` window the daemon owns, and `LinkTouch` and
  `LinkGlobal` go back; the desktop app's Mirror control prefers it and
  falls back to `adb` (`MAG-P6-A` through `-D`).
- Registered commands (name, program, arguments) live in the daemon's
  configuration and reach the phone as ids and names; a run is a bounded
  process group. The phone's trackpad and keyboard drive one virtual
  `uinput` device the daemon owns, through a rate governor
  (`MAG-P5-A`, `-B`).
- Contacts, SMS and calls reach the desktop over the own wire: the contact
  book names numbers, conversations and threads show on the desktop app's
  messages page and replies go back, received messages and calls become
  notifications with reply and buttons, and `Devices1` publishes the call
  state and the SMS methods (`MAG-P4-E`, `-F`, `-G`).
- The phone's player shows on the desktop's media card over the own wire
  and the desktop's buttons drive it; the desktop's MPRIS players reach
  the phone through the same playerctl worker while it asks (`MAG-P4-D`).
- Files travel both ways on the own wire on their own streams and resume
  after a broken link; received files are published in the downloads
  directory only while the device is still paired (`MAG-P4-C`).
- The phone's notifications show on the desktop over the own wire with
  their buttons; the desktop's presses, replies and dismissals go back,
  also through `Devices1.NotificationAction`, `ReplyNotification` and
  `DismissNotification` (`MAG-P4-B`).
- The clipboard travels both ways on the own wire: the desktop's changes
  drain into the phone's session, the phone's text is written through the
  Wayland adapter, and the daemon asks for the phone's clipboard when a
  session opens (`MAG-P4-A`).
- The desktop app shows the pairing QR: its pairing action arms the
  daemon's window, draws the code, counts the two minutes down and names
  the phone once it arrives (`MAG-P3-B`). A paired, connected own-wire
  phone reads as connected, not connecting.
- The S25U pairs with the daemon by scanning its QR, holds the session
  through screen off, app switch and Wi-Fi toggle, and rings on `Ring`;
  the daemon answers a QR proof from a phone it still pins, so a phone
  that forgot it can pair again (`MAG-P3-C`).
- Delivered as `1.2.2`: `FEEDBACK-1-MAG`. A plugin row and its switch lit at
  the same time whenever the pointer was on the switch; the row now paints the
  shared `CelestinaRowHighlight` and yields its hover to the switch. "Olvidar"
  and "Vincular" are the `unlink` and `link` glyphs with the words in their
  accessible names; the mirror start, the mirror settings and the mirror
  choice segments are `checkable` — re-binding `checked` on click, as the
  plugin switch does, so the button still shows only what the daemon
  confirms. The choice labels stay words because they are the values. Hand
  check: `VAL-MAG-10`.
- The checkout is clean of Magnetita work: `MAG-S1`, `MAG-R1` and `MAG-R2` are
  all committed.
- The installed daemon carries `MAG-S1`, `MAG-R1` and `MAG-R2`'s port pinning
  and remembered endpoint, but **not** `MAG-R2`'s stale-advertisement fallback:
  `verify-production.sh` runs a workspace-wide `cargo fmt --all --check` and
  unrelated in-flight `grafita-core` work was failing it when that correction
  landed. Redeploying is the only thing standing between the tree and the
  installed bytes.
- The mirror was observed working end to end against the real S25U, including
  pairing on six digits alone and pinning to the fixed port. What has never been
  observed is a mirror after a *phone* reboot (`VAL-MAG-09`).

- Until `MAG-P7-C`, `magnetitad` implemented KDE Connect discovery, TCP/TLS trust, local pairing,
  storage mount, the daily plugins and `org.celestina.Devices1`.
- Siderita consumes the mount/device/media contract; the Celestina shell
  consumes phone/battery state; the standalone Magnetita app owns pairing,
  diagnostics and settings UI.
- Pairing v8 timestamp/code validation, identity binding, bounded admission,
  durable revocation, payload publication barriers and typed MPRIS actions are
  implemented and covered by unit/loopback evidence.
- App actions use one ordered owned worker. Snapshot reads/watchers remain
  detached best-effort work and still lack a fully deterministic shutdown path.
- Live phone evidence for the released daily set predates the 2026-07-29
  hardening; it is not reused as proof of the corrected paths.
- `SendFileUri` on
  `org.celestina.Devices1`. It names the file by the percent-encoded `file://`
  URI the portal and the clipboard already speak, decodes it by bytes with
  `celestina_core::percent`, and refuses a URI that is not a local `file://` one
  or whose escapes are malformed with a typed reason. `Command::SendFile` now
  carries a `PathBuf`, so a filename that is not valid UTF-8 reaches
  `serve_file` unaltered. `SendFile` itself is unchanged and stays for
  compatibility: it is a published interface and altering the meaning of its
  argument would break any other caller. Siderita's send-to-phone menu item is the
  first consumer, under its own `SID-G7-G`. Committed and deployed; no live file
  transfer has been observed since — that is `VAL-MAG-HARDENING`. See the
  [byte-exact send evidence](docs/evidence/2026-08-06-byte-exact-send-to-phone.md).
- The `MAG-S1` corrections are committed and installed. They have never been
  exercised against the phone: `VAL-MAG-06` is what would prove the corrected
  boundaries hold, and it has not been run.

## Planned implementation debt

- Give the app's detached read/watch side explicit ownership, cancellation and
  deterministic join without regressing burst coalescing (`MAG-M1`).
- Keep packaging and resource diagnostics aligned with the canonical production
  artifact workflow. Service activation remains deploy-only.
- The own-protocol program (`MAG-P0` through `MAG-P7`, accepted in
  [ADR 0001](docs/decisions/0001-own-protocol-and-android-app.md) on
  2026-09-04) is planned and not started: a private QUIC protocol shared by
  both ends through one Rust crate, a Kotlin/Compose Android application,
  remote input, SMS, contacts and telephony, and the mirror as a capability
  of the link. Its [five discussions](docs/discussions/README.md) are
  concluded and applied; the `MAG-P0` spikes verify them. The KDE Connect
  wire and the `adb`/`scrcpy` mirror are removed in `MAG-P7`. The drawing
  tablet and presenter stay out by the author's choice.

## Blockers

No implementation blocker is recorded. The installed daemon carries
everything committed through `3419cff`; `celestina/scripts/complete-production.sh`
is still owed for the shell bundle's copy of `magnetita-core`, unchanged since
`MAG-S1`. No other
implementation blocker is recorded. The real phone/network is required only
for the independent validation queue.

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
- [Active plan MAG-P7](docs/plans/active/2026-09-10-storage-and-retirement.md)
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
