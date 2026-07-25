# Magnetita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the phone
> link only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is not
> "verified": pairing and every plugin must be proven against a real device on a
> real network, tracked as its own goal. `magnetita-core` has begun — the packet
> envelope and the identity packet, offline-tested; everything past the core
> awaits the real phone.

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
(`deviceId`, `deviceName`, `deviceType`, `protocolVersion` `7`, `tcpPort`, and
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
- **The crypto/async closure is the one deliberately expensive dependency.**
  TLS (`rustls`), self-signed certs (`rcgen`) and an async runtime are the
  heaviest closure the suite has taken on. It is inherent (you cannot speak TLS
  to a phone cheaply; Valent and KDE Connect pay it too), earned by a proven
  daily need, and amortized as shared session infrastructure like the Qt
  runtime. It is measured, not smuggled: closure size **and idle wakeups** (a
  long-lived service) are in the budget. `unsafe_code` stays forbidden (workspace
  lint) — `rustls`/`rcgen` keep us in safe Rust.
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
- [ ] `magnetita-core` — the `kdeconnect.pair` state machine (request / accept /
      reject / ~30 s timeout), pure and unit-tested
- [ ] `magnetita-net` — UDP identity broadcast + listen; TCP accept/connect
- [ ] `magnetita-net` — TLS upgrade with a self-signed cert (`rcgen`) and a
      custom `rustls` verifier implementing TOFU pinning; per-device trust store
      persisted on disk
- [ ] `magnetita-net` — structured connection events and a *reason* on every
      failure (no reply, cert changed, pairing rejected/timed out), so CP1's log
      has something truthful to show
- [ ] `magnetitad` — CLI service: discover, pair, ping round-trip, clean
      shutdown/join of the async runtime
- [ ] **Verified** — pairs with the stock Android app and pings both ways on a
      real network; reconnect re-verifies the pinned cert; unpair is honored
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

- [ ] **Daemon↔UI IPC** — the convention this project forces (the app is a client
      of the service): the device list, the connection event stream, pair/unpair
      and option toggles, over one activation-capable contract that becomes the
      suite's reference for it
- [ ] **The app** — `magnetita-qt` + a Qt/QML host over `celestina-style`: the
      device list (discovered + paired), pair/unpair with the verification key,
      and per-device / per-plugin options — the ones that are Magnetita's, not
      Siderita's
- [ ] **The connection log** — the live event stream and the last failure per
      device, in plain language ("no reply on 1716", "certificate changed",
      "pairing timed out", "on a different network"), so a phone that will not
      connect is diagnosable *in the app*, not in the journal
- [ ] **Runs in the background with a window on demand** — closing the window
      leaves the service (and the mount) up; opening it re-attaches to live state
- [ ] **Verified** — pairing, unpair and a forced failure (wrong network,
      rejected pairing) each read correctly in the app against the real device

**Done when:** the author opens Magnetita, pairs the phone from the window, and —
when it will not connect — the window tells them why, in words.

## Checkpoint 2 — The phone in Siderita (the first cross-app experience)
**Goal:** a paired phone, mounted, appears in Siderita's sidebar and you browse
it — the integration that is the reason Magnetita comes first among the new apps.
Built on CP0's channel; still service-side.

- [ ] `magnetita-core` — the `kdeconnect.sftp` plugin bodies: request the mount
      info, parse the reply (host, port, user, credentials, path roots). Pure,
      unit-tested
- [ ] `magnetitad` — mount the phone's SFTP with sshfs/FUSE at
      `$XDG_RUNTIME_DIR/magnetita/<device-id>/`, **kept mounted** while the device
      is connected and paired, unmounted cleanly on disconnect or shutdown
- [ ] `magnetitad` — serve `org.celestina.Devices1` on the session bus: the
      connected devices with id, name, type, battery, connection/mount state and
      mount path; the contract is **versioned and documented** for Siderita to pin
- [ ] **Siderita** — consume `org.celestina.Devices1` in the sidebar: draw the
      phone under a devices section, open its mount path on click, reflect the
      *connecting → mounting → mounted* state truthfully. Siderita's first time
      *consuming* a suite contract rather than serving one
- [ ] **Verified** — against the real phone: the mount appears, survives a
      reconnect, and Siderita browses it; an unpair or a lost link removes it from
      the sidebar without stranding a dead mount
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
