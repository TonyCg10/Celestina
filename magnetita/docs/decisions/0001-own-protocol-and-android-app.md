# ADR 0001: Magnetita grows its own protocol and its own Android application

- **Date:** 2026-09-04
- **Status:** accepted
- **Accepted:** 2026-09-04, with the five discussions concluded the same day;
  the `MAG-P0` spikes verify the choices rather than gate them

## Context

Magnetita 1.x speaks KDE Connect to the stock Android client. That bought a
working phone link without writing a phone side, and it also fixed every
limit the suite has since measured: phone-to-desktop clipboard is manual
because the stock client cannot read the clipboard in the background; the
mount is an `sshfs` subprocess fed by peer-chosen strings (`MAG-S1` closed the
resulting command-execution hole); the wire is newline-delimited JSON over one
TLS stream, so one large notification body or file stalls everything behind
it; and the screen mirror is not part of the link at all — `MAG-R1`/`MAG-R2`
drive `adb` and an external `scrcpy` over Android's wireless debugging, which
Android switches off on every phone reboot.

The author has decided that the next step is a private protocol and a
first-party Android application in Kotlin, with the KDE Connect features the
author actually uses (clipboard, notifications, run-commands, trackpad and
keyboard, battery, find-my-phone, file share, media control), the phone's
own communication surface (SMS, contacts, telephony), and with the mirror as
an integral capability of the same protocol rather than a separate tool.
Features the author does not use — drawing tablet, presenter — are out.

The suite's rules shape the answer more than taste does: every invariant has
one owner, pure protocol logic lives in `celestina-rs`, network input is
hostile, and `org.celestina.Devices1` is a published contract that Siderita
and the shell consume and that must not break.

## Decision

1. **One protocol implementation, shared by both ends.** The protocol's pure
   half (`magnetita-proto`: messages, framing, capability negotiation, pairing
   state machines, validation) and its transport (`magnetita-link`: QUIC with
   mutual TLS, trust store, discovery types, reconnection) are Rust crates in
   `celestina-rs`. The Android application does not reimplement them in
   Kotlin: a third crate, `magnetita-mobile`, exposes them through UniFFI as a
   `cdylib` built with `cargo-ndk`. The Kotlin side is UI, Android platform
   adapters and lifecycle — the same "thin client over a Rust core" shape the
   desktop applications already have.

2. **Transport is QUIC over UDP with mutual TLS 1.3**, using `quinn` on the
   `rustls`/`ring` stack the daemon already depends on. Each device holds one
   self-signed certificate; pairing pins the peer's fingerprint and nothing but
   that fingerprint authenticates a connection afterwards. Streams give each
   capability its own ordering domain: one bidirectional control stream for
   small messages, one stream per file transfer, unidirectional streams for
   mirror video and audio, and QUIC datagrams for pointer motion where
   dropping a stale sample beats delivering it late. Connection migration
   keeps a session across the phone's address changes. Concluded in the
   [transport discussion](../discussions/2026-09-04-transport-quic-or-tcp.md);
   `MAG-P0-A` verifies the stack on the phone.

3. **Messages are CBOR with integer keys** inside a versioned envelope
   (`{version, capability, kind, id, body}`). Unknown keys are ignored,
   unknown capabilities are declined in the hello, and every peer-chosen
   string, size and count is bounded at decode — the `MAG-S1` discipline is a
   property of the new decoder, not a patch on it.

4. **Discovery is mDNS on both sides**: the daemon advertises and browses
   `_magnetita._udp` through Avahi (over the `zbus` the mirror discovery
   already uses); the phone uses `NsdManager`. Either side may dial; the
   fingerprint, not the address, decides trust.

5. **Pairing is out-of-band by default.** The desktop shows a QR code carrying
   its device id, certificate fingerprint, reachable addresses and a one-time
   secret; the phone scans it, dials, and both sides prove possession of the
   secret over the encrypted channel before pinning each other. The fallback
   is a six-digit code turned into a shared key with SPAKE2, for a headless
   peer or a desktop without a window; both paths end in the same pin.
   Concluded in the
   [pairing discussion](../discussions/2026-09-04-pairing-qr-or-code.md).

6. **Capabilities, not plugins.** The hello lists capability ids with versions.
   The set is `battery`, `clipboard`, `notifications`, `find`, `share`,
   `media`, `commands`, `input`, `mirror`, and the phone's communication
   surface: `sms` (conversation list, thread history, send, receive, MMS
   attachments as `share` streams), `contacts` (one-way sync of the phone's
   contacts to the desktop as vCard 4.0, used by `sms` and `telephony` to
   name numbers) and `telephony` (ringing, answered, missed and ended
   events; mute the ringer; answer and hang up through Android's
   `TelecomManager`). `storage` (browsing the phone from Siderita without
   `sshfs`) closes the set in `MAG-P7`.

7. **The mirror is a capability of the link.** The phone captures its screen
   with `MediaProjection`, encodes with `MediaCodec` (HEVC where the hardware
   has it, H.264 otherwise), and sends the elementary stream on a QUIC stream;
   the desktop decodes through the `fluorita-engine` `libmpv` seam, as a
   bounded exception under
   [ADR 0005](../../../docs/decisions/0005-bounded-qt-bridge-crates.md), and
   shows it in a window the daemon owns, sending `input` events back through
   the phone's accessibility service. The `adb`/`scrcpy` path from
   `MAG-R1`/`MAG-R2` stays installed and unchanged through `MAG-P6` and is
   retired in `MAG-P7` once `VAL-MAG-14` observes the own mirror after a
   phone reboot; it is not kept as a second mode. Concluded in the
   [mirror discussion](../discussions/2026-09-04-mirror-capture-path.md).

8. **The desktop contract does not change.** `magnetitad` hosts the new link
   next to the KDE Connect one and keeps publishing `org.celestina.Devices1`
   and `org.celestina.Mirror1`; Siderita and the shell see the same snapshots.
   The daemon's existing plugin modules (clipboard, notifications, media, file
   share, find) keep their domain and gain a second wire. The KDE Connect
   wire, its pairing and the `sshfs` mount are removed in `MAG-P7`, after
   `storage` replaces the mount and `VAL-MAG-12` through `VAL-MAG-14` record
   daily use of the own app. Concluded in the
   [compatibility discussion](../discussions/2026-09-04-kde-connect-compatibility.md).

9. **The Android application is a registered project**, `magnetita-android/`,
   with its own README/STATUS/ROADMAP/VALIDATION set, commit prefix
   `magnetita-android`, a Gradle version source, and build/verify/deploy
   scripts that consume one signed artifact. Kotlin 2.x, Jetpack Compose with
   Material 3, coroutines and `Flow`, `DataStore`, foreground services with
   declared types, `minSdk 31`, `targetSdk` current. Product copy is Spanish
   under [ADR 0007](../../../docs/decisions/0007-spanish-product-copy.md).

10. **Remote input on the desktop is injected through `uinput`**, owned by the
    daemon with the same bounded, typed, pid-owned discipline as its
    subprocesses, and exercised only against the development nest — never the
    author's live session. The udev rule is a recorded host change in
    `HOST-HYGIENE.md`. The RemoteDesktop portal with `libei` is the named
    successor, taken as soon as the author's compositor exposes it; see the
    [input discussion](../discussions/2026-09-04-remote-input-on-wayland.md).

11. **The phone's communication surface is data the desktop reads, and
    actions the phone performs.** The app needs `READ_SMS`, `SEND_SMS`,
    `RECEIVE_SMS`, `READ_CONTACTS`, `READ_PHONE_STATE`, `READ_CALL_LOG` and
    `ANSWER_PHONE_CALLS`; it is sideloaded and never published on a store
    that restricts them. Message bodies, names and numbers are bounded
    peer-chosen text rendered as plain text on the desktop, stored only in
    the daemon's memory for the session, and never written to the desktop
    disk unless the author enables a per-capability setting. Sending an SMS
    is a phone-side action that the desktop requests by conversation id and
    text; the phone is the only side that knows a number.

## Consequences

- Magnetita's user contract changes: it becomes an Android application and a
  private protocol, which its README said it was not. The README's user
  contract is rewritten with this acceptance.
- Two wires coexist in `magnetitad` for the whole program. The transition is
  bounded by `MAG-P7`; the daemon never carries three.
- The Rust core must build for `aarch64-linux-android`; `cargo-ndk`, an NDK
  and UniFFI join the toolchain. The Android build is a separate artifact
  with its own signing key, kept outside the repository.
- `celestina-rs` gains three crates with one consumer each; they are
  justified by the stable tested boundary the shared protocol is, not by
  consumer count.
- Everything the phone side proves in tests is proven by the same crate the
  daemon uses, so interoperability is by construction, and a headless peer
  simulator built from the same crate stands in for the phone in daemon
  tests.
- The mirror without `adb` costs a consent dialog on the phone on every
  capture session (Android 14+), shows secure surfaces as black, and gains
  input only through an accessibility service; `MAG-P0` measures the cost so
  it is known, and the `scrcpy` path is not touched until `MAG-P7`.
- SMS, contacts and call state are the most personal data the suite will
  ever carry; they ride the same pinned, encrypted link as everything else,
  and the desktop keeps none of it at rest by default. Android's default-SMS
  requirement does not apply to reading and sending through `SmsManager`
  with the granted permissions, but MMS and RCS are limited to what the
  provider exposes; RCS is out.

## Revisit when

- `MAG-P0-A` shows `quinn` or `rustls`/`ring` cannot be built or perform
  acceptably on the phone; the transport falls back to TCP + TLS with an
  in-house multiplexer over the same message layer.
- niri exposes the RemoteDesktop portal with `libei`; input moves from
  `uinput` to the portal and the udev rule is withdrawn.
- The author finds the camera slower than six digits; the typed path
  becomes primary.
- The measured glass-to-glass latency of the `MediaProjection` mirror or the
  accessibility-service input on the S25U is judged unusable by the author;
  the `scrcpy` path then stays as the mirror and the capability is dropped.
- Android restricts `NotificationListenerService`, `MediaProjection` or
  clipboard access further in a way the app cannot satisfy.
- A second consumer of the protocol crates appears (another desktop, a
  tablet), which would justify a public wire specification.
