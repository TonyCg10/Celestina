# Magnetita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the phone
> link only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is not
> "verified": pairing and every plugin must be proven against a real device on a
> real network, tracked as its own goal. **CP0, CP1 and CP2 are done.** The
> from-scratch Rust transport (`magnetita-net`) and the `magnetitad` daemon pair
> live and stably with the real phone (a Galaxy S25 Ultra, protocol 8), reconnect
> as already-trusted, run as a systemd user service, mount the phone's storage
> over sshfs, and serve `org.celestina.Devices1` — which **Siderita** consumes to
> draw the phone in its sidebar (click to browse it) and the standalone
> **Magnetita app** consumes to pair/unpair with the verification key and show the
> connection log. ~66 offline tests, **no async runtime**, no C toolchain,
> `unsafe` forbidden. CP3 — the daily plugins (notifications, share, clipboard,
> battery, find-my-phone) — is next.

## Overview

**Purpose.** The suite's phone link — connect the desktop to a phone over the
LAN and sync the things a daily user actually reaches for. It is two things at
once: a **standalone app** (like Valent's window — pair a device, see *why* it
will not connect, set its options) and the **first cross-app integration** in the
suite (the phone, mounted, browsable in Siderita). File share, mirrored
notifications, clipboard, battery and find-my-phone follow. One native,
first-party stack the whole session owns end-to-end rather than a foreign desktop
client. No SMS bridge, no contacts store, no remote-input server until a daily
gap proves each one.

**What it replaces, and what it doesn't.** Magnetita speaks the **KDE Connect
network protocol**, an open, multiply-implemented de-facto standard (kdeconnect,
GSConnect, Valent, mconnect). So it interoperates with the stock **FOSS KDE
Connect Android app** and with other KDE Connect desktops. The "third app" it
drops is only the *desktop* daemon (Valent / `kdeconnectd` and their GTK/KDE
dependency closures). The phone client stays the reference Android app; Magnetita
does **not** include an Android app of its own. Interoperability on the wire is a
hard contract — Magnetita never forks the protocol into private glue.

**The protocol, briefly.** Four layers: (1) **discovery** — an identity packet
(`deviceId`, `deviceName`, `deviceType`, `protocolVersion` `8`, `tcpPort`, and
`incomingCapabilities`/`outgoingCapabilities`) broadcast over UDP on port 1716
(range 1716–1764); a peer connects back over TCP. (2) **packets** —
line-delimited JSON `NetworkPacket`s `{ id, type: "kdeconnect.<name>", body }`,
one `type` per plugin, sent only for types the peer declared it accepts. (3)
**trust** — the TCP socket upgrades to TLS 1.2+ with self-signed certs; pairing
is a `kdeconnect.pair` exchange (`{"pair": true}`, ~30 s timeout) after which
both ends **pin** the peer certificate (trust-on-first-use, like SSH). (4)
**payloads** — bulk data (files) is streamed over a *separate* TLS connection
named by `payloadSize` + `payloadTransferInfo`.

**Shape.** A **background service** (`magnetitad`, systemd *user* service, no Qt)
that holds the connections, keeps the phone mounted and serves
`org.celestina.Devices1`, plus a **standalone Qt/QML app** — a first-class window
like Valent's, not an optional client. The service keeps *connected and mounted*
true with the window closed (Siderita still sees the phone); the app is how a
human pairs, diagnoses and configures. This splits Magnetita into two processes
and makes it the first project to need the suite's **daemon↔UI IPC/activation
convention** — an item the suite roadmap parks under its Checkpoint 2, pulled
forward here.

**Key decisions.**
- **Speak KDE Connect on the wire.** Reimplementing a private protocol *and* an
  Android client is out of scope and off-principle; interop with the reference
  client is the contract.
- **The phone reaches Siderita as a real mount plus a versioned contract.** Its
  storage is mounted over the KDE Connect SFTP plugin (sshfs) at a stable path
  under `$XDG_RUNTIME_DIR/magnetita/` and kept mounted while connected, so
  browsing it is plain filesystem navigation; the device — name, type, battery,
  state, mount path — is served on `org.celestina.Devices1`, the suite's **first
  internal contract**, which Siderita consumes. The filesystem carries the bytes,
  the contract what it cannot. (CP2.)
- **Connection is legible.** The service emits a structured event for every step
  (discovery seen, TCP open, TLS handshake, pairing, mount) and a *reason* on
  every failure, and the app shows them as a connection log. "Why won't my phone
  connect" has an answer in the window, not in journald — a deliberate
  improvement over the opaque failure of the tools it replaces.
- **Rust core, thin bridge, QML UI** — the suite stack. Protocol domain and
  transport are pure Rust and testable without Qt or a live phone; the app is a
  CXX-Qt client over `celestina-style`.
- **The crypto closure is the one deliberately expensive dependency — and it
  came in leaner than feared.** TLS (`rustls` with the `ring` provider) and
  self-signed certs (`rcgen`) are the heaviest closure the suite has taken on. It
  is inherent (you cannot speak TLS to a phone cheaply; Valent and KDE Connect pay
  it too), earned by a proven daily need, and amortized as shared session
  infrastructure. Kept lean at CP0: **no async runtime** — blocking `std::net` on
  a thread, not tokio (one phone does not need a reactor) — and **no C toolchain**
  (`ring`/`rcgen` pinned to their pure-Rust builds, never aws-lc-rs/cmake), so a
  full TLS stack is ~15 crates. It is measured, not smuggled: closure size **and
  idle wakeups** (a long-lived service) are in the budget. `unsafe_code` stays
  forbidden (workspace lint) — `rustls`/`rcgen`/`ring` keep our code in safe Rust.
- **Trust-on-first-use with a shown verification key.** Certs are pinned on
  pairing and verified on every reconnect; the pairing surface shows a key
  derived from both certificates so a human can confirm no MITM.
- **Standards to the desktop.** Phone notifications emit through
  `org.freedesktop.Notifications`; shared-in files land in an XDG dir Siderita
  shows; "send to phone" is an `open-with` handler. No private desktop glue.
- **Compose, then replace.** Valent stays in autostart until the daily set is
  verified, then leaves deliberately — the suite's earned-replacement discipline.

## Checkpoint 0 — A trusted channel (prove the hard part first)
**Goal:** `magnetitad` discovers and is discovered by the real KDE Connect
Android app on the LAN, completes trust-on-first-use pairing, holds a TLS 1.2+
channel across reconnects, and round-trips `kdeconnect.ping` in both directions —
driven from a CLI, with no UI and no plugins. This proves the riskiest layer
(discovery + the mutual self-signed TLS/TOFU handshake against the reference
client) before anything is built on top of it.

- [x] `magnetita-core` — `NetworkPacket` (de)serialization, the identity packet,
      capability sets and the `deviceId`/type model: pure Rust, unit-tested
      without I/O (8 tests)
- [x] `magnetita-core` — the `kdeconnect.pair` state machine (request / accept /
      reject / ~30 s timeout) and the pair packet body: pure, returning a
      `PairAction` the transport runs, with no clock; 13 unit tests over every
      flow (mutual request, rejection, both timeouts, unpair, restore)
- [x] `magnetita-core` — the device session (peer identity + pairing → a
      `Reaction` of packets to send and events to log), the connection-event
      vocabulary the log reads, and our desktop identity/capabilities: pure, no
      clock, 11 more tests. The brain decides; the transport does the I/O
- [x] `magnetita-net` — UDP identity broadcast + listen (`discovery`); TCP
      connect (`link::connect`, the connector/TLS-server role) and accept
      (`link::accept`); the exact KDE Connect **v8** handshake — plaintext identity
      with `targetDeviceId`, then encrypted re-exchange for protocol ≥ 8
- [x] `magnetita-net` — TLS upgrade with a self-signed cert (`rcgen`) and custom
      `rustls` verifiers doing real handshake-signature checks but authority-free
      TOFU pinning (`tls`, `cert`); per-device trust store persisted on disk
      (`trust`, three verdicts: trusted / unknown / changed-and-refused)
- [x] `magnetita-net` — structured connection events (`ConnectionEvent`) and a
      typed *reason* on every failure (`LostReason`: no reply, cert changed,
      pairing rejected/timed out), so CP1's log has something truthful to show
- [x] `magnetitad` — headless service: discover, dial, pair (phone-driven,
      auto-accepted), ping send; blocking-thread runtime, no async to join
- [x] **Verified (pairing + reconnect)** — pairs with the stock Android app on a
      real network (Galaxy S25 Ultra) and holds a stable link; a restart reconnects
      and **re-verifies the pinned cert** as already-trusted with no re-pair; a
      changed cert would be refused; unpair is forgotten from the trust store
- [ ] **Verified (ping both ways)** — confirm `kdeconnect.ping` is seen on the
      phone and one sent *from* the phone is logged (send path is live; the
      round-trip is the last CP0 check)
- [ ] **Measured** — installed closure size and service idle wakeups reported and
      inside a declared budget

**Done when:** an unpaired phone and this service reach a mutually-trusted,
reconnect-stable TLS channel and exchange pings, verified against the reference
client — and the transport's cost is a number, not a hope.

## Checkpoint 1 — The app: pair, diagnose, configure
**Goal:** the standalone window — like Valent's, and a first-class surface — on
top of CP0's channel: pair a device with the shown verification key, read a
**connection log** that says *why* one will not connect, and set the options that
are not the file integration. The service stays headless underneath; this is the
human surface, and the first use of the suite's daemon↔UI convention.

- [x] **Daemon↔UI IPC** — `org.celestina.Devices1` on the session bus is the
      contract: `ListDevices` / `RecentLog`, `RequestPair` / `Unpair`, and
      `Changed` / `Event` signals. The app is a pure client; the daemon holds all
      state. The suite's reference for daemon↔UI.
- [x] **The app** — a `magnetita` Qt/QML window over `celestina-style` (lean: zbus
      client, no C++ shims): the device list, pair / unpair with the shown
      verification key, "Abrir" to browse. Per-plugin options wait for plugins
      (CP3) — there is nothing to configure yet, honestly, so nothing is shown.
- [x] **The connection log** — an ACTIVIDAD panel: the live event stream (off the
      `Event` signal) and every failure in red in plain language ("sin respuesta",
      "el certificado cambió", "el emparejamiento expiró", "inalcanzable (¿otra
      red?)"), so *why it won't connect* is answered in the window, not the journal
- [x] **Runs in the background with a window on demand** — the daemon is a separate
      systemd service; closing the window leaves it (and the mount) up, proven
      live; opening the window re-reads live state (the app is stateless)
- [x] **Verified** — against the real phone: pair, unpair (drops trust, re-pair
      works), and the log renders milestones and would render a failure in red;
      installed as an app (`~/.local/bin` + a `.desktop` entry)

**Done when:** the author opens Magnetita, pairs the phone from the window, and —
when it will not connect — the window tells them why, in words. **Done.**

## Checkpoint 2 — The phone in Siderita (the first cross-app experience)
**Goal:** a paired phone, mounted, appears in Siderita's sidebar and you browse
it — the integration that is the reason Magnetita comes first among the new apps.
Built on CP0's channel; still service-side.

- [x] `magnetita-core` — the `kdeconnect.sftp` plugin bodies: request the mount
      (`kdeconnect.sftp.request`), parse the reply into a typed `SftpMount` (port,
      one-session user/password, root path, `multiPaths`/`pathNames`) or an
      `SftpReply::Error`. Pure, 7 unit tests
- [x] `magnetitad` — mount the phone's SFTP with sshfs/FUSE at
      `$XDG_RUNTIME_DIR/magnetita/<device-id>/`, **kept mounted** while connected,
      unmounted on disconnect (`Mount` drops → unmount) and swept at startup after
      a killed run. Needed the identity to advertise the plugin — a peer only
      sends a type the other lists as incoming
- [x] `magnetitad` — serve `org.celestina.Devices1` on the session bus:
      `ListDevices() -> aa{sv}` (id, name, type, connected, mounted, mountPath,
      battery) plus a `Changed` signal, backed by a shared registry the links
      update. Best-effort, on zbus (the suite's D-Bus stack)
- [x] **Siderita** — consumes `org.celestina.Devices1`: a "MÓVIL" section in the
      sidebar, click opens the mount, a still-mounting phone shows dimmed as
      "conectando…". Live-refreshed off the `Changed` signal. Siderita's first
      time *consuming* a suite contract rather than serving one
- [x] **Verified** — against the real phone (Galaxy S25 Ultra): the phone mounts,
      its storage browses, a reconnect re-mounts, and it appears under MÓVIL in
      Siderita and opens on click
- [ ] **Measured** — the always-on mount's idle cost (wakeups, memory) is a
      number inside the budget

**Done when:** the author opens Siderita and the phone is simply *there*,
browsable like a USB stick, and the state it shows is honest.

## Checkpoint 3 — The daily plugins (earn Valent's retirement)
**Goal:** the plugins that make it a daily driver, each chosen by what the author
actually uses in Valent, wired through freedesktop standards and surfaced in the
app's options.

- [ ] **Notifications** — phone notifications mirror to the session's own
      notification daemon (`org.freedesktop.Notifications`), with dismiss and,
      where the phone supports it, quick-reply
- [ ] **Share, both directions** — phone → PC payload transfer into an XDG dir
      Siderita shows; PC → phone as a `share.request` (file / URL / text)
- [ ] **Clipboard** — opt-in text clipboard sync
- [ ] **Battery & connectivity** — phone battery and cell/connectivity status
      (also carried on `org.celestina.Devices1` for the sidebar and shell)
- [ ] **Find-my-phone** — ring the device
- [ ] **Verified** — every plugin exercised against the real device; a source
      file is never removed before its transferred copy is verified (the suite's
      loss-free rule, shared with `siderita-ops`)

**Done when:** the author runs the session with Valent removed from autostart
because Magnetita covers the daily set, proven on a real phone.

## Checkpoint 4 — One suite (stop being an island)
**Goal:** the service, the app and the shell behave as one suite, in one visual
language.

- [ ] **Shell surfacing** — connected device + battery in the `celestina` panel
      (notifications already flow via freedesktop)
- [ ] **Send to phone in Siderita** — an entry in Siderita's context menu via
      `open-with`, complementing the browse-the-phone mount of CP2
- [ ] **One settings source** — paired devices and per-plugin toggles share the
      suite's settings/theming source, not a private store
- [ ] **Media control (MPRIS)** — control phone/desktop playback both ways

**Done when:** pairing, status, transfers, the mount and settings are reachable
from the suite's own surfaces in the suite's own visual language, over the suite's
own contracts.

## Later / someday
- [ ] SMS/telephony, contacts, remote input (`mousepad`), run-command, system
      volume, presentation remote — each only after a real daily gap appears,
      never for KDE Connect parity
- [ ] mDNS discovery alongside UDP broadcast, and newer protocol versions, if/when
      the reference client requires them
- [ ] Packaging: systemd user-unit install, first-pairing docs, dependency and
      resource diagnostics

## Non-goals
- **No Android app.** The phone side stays the FOSS KDE Connect client.
- **No private protocol.** Wire-interoperability with KDE Connect is a contract;
  Magnetita does not fork it into glue.
- **No feature parity.** Plugins are earned by use, not by matching KDE Connect's
  list; a plugin count is not progress.
- **No reimplementation of what the suite already has.** Magnetita routes into
  the notification daemon, the file manager and the media player through
  freedesktop and the suite's contracts; it does not embed them.
- **No heavy frameworks.** The crypto/async closure is the single measured
  exception; nothing else is added without a demonstrated, measured need.
- **Not a general product.** Like the rest of Celestina, this is for its author's
  session, not a cross-desktop KDE Connect replacement for others.
