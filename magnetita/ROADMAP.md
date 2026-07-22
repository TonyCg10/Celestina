# Magnetita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the phone
> link only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is not
> "verified": pairing and every plugin must be proven against a real device on a
> real network, tracked as its own goal. Nothing here is built yet — this is a
> design-stage roadmap.

## Overview

**Purpose.** The suite's phone link — connect the desktop to a phone over the
LAN and sync the things a daily user actually reaches for: mirrored
notifications, file share both directions, clipboard, battery/connectivity and
find-my-phone. One native, first-party daemon the whole session syncs through,
so the phone integration is owned end-to-end rather than delegated to a foreign
desktop client. No SMS bridge, no contacts store, no remote-input server until a
daily gap proves each one.

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

**Shape.** A **headless daemon** (`magnetitad`, systemd *user* service, no Qt)
plus an **optional Qt/QML UI client**. The daemon holds the connections and
routes into the rest of the desktop over freedesktop standards; the UI is a thin
client for pairing, status and transfers, and the session works without it. This
is a different shape from Siderita (a windowed app) and it makes Magnetita the
first project to need the suite's **daemon↔UI IPC/activation convention** — an
item the suite roadmap parks under Checkpoint 2, pulled forward here.

**Key decisions.**
- **Speak KDE Connect on the wire.** Reimplementing a private protocol *and* an
  Android client is out of scope and off-principle; interop with the reference
  client is the contract.
- **Rust core, thin bridge, QML UI** — the suite stack. Protocol domain and
  transport are pure Rust and testable without Qt or a live phone; the UI is a
  CXX-Qt client over `celestina-style`.
- **The crypto/async closure is the one deliberately expensive dependency.**
  TLS (`rustls`), self-signed certs (`rcgen`) and an async runtime are the
  heaviest closure the suite has taken on. It is inherent (you cannot speak TLS
  to a phone cheaply; Valent and KDE Connect pay it too), earned by a proven
  daily need, and amortized as shared session infrastructure like the Qt
  runtime. It is measured, not smuggled: closure size **and idle wakeups** (a
  long-lived daemon) are in the budget. `unsafe_code` stays forbidden
  (workspace lint) — `rustls`/`rcgen` keep us in safe Rust.
- **Trust-on-first-use with a shown verification key.** Certs are pinned on
  pairing and verified on every reconnect; the pairing surface shows a
  key derived from both certificates so a human can confirm no MITM.
- **Standards to the desktop.** Phone notifications emit through
  `org.freedesktop.Notifications`; inbound files land in an XDG dir Siderita
  shows; "send to phone" is an `open-with` handler. No private desktop glue.
- **Compose, then replace.** Valent stays in autostart until CP1 is verified,
  then leaves deliberately — the suite's earned-replacement discipline.

## Checkpoint 0 — A trusted channel (prove the hard part first)
**Goal:** `magnetitad` discovers and is discovered by the real KDE Connect
Android app on the LAN, completes trust-on-first-use pairing, holds a TLS 1.2+
channel across reconnects, and round-trips `kdeconnect.ping` in both directions —
driven from a CLI, with no plugins and no Qt. This proves the riskiest layer
(discovery + the mutual self-signed TLS/TOFU handshake against the reference
client) before anything is built on top of it.

- [ ] `magnetita-core` — `NetworkPacket` (de)serialization, identity, capability
      sets, `deviceId`/type model, and the pairing state machine, all pure and
      unit-tested without I/O
- [ ] `magnetita-net` — UDP identity broadcast + listen; TCP accept/connect
- [ ] `magnetita-net` — TLS upgrade with a self-signed cert (`rcgen`) and a
      custom `rustls` verifier implementing TOFU pinning; per-device trust store
      persisted on disk
- [ ] `magnetita-net` — `kdeconnect.pair` accept/reject with the ~30 s timeout
      and a verification key surfaced to the caller
- [ ] `magnetitad` — CLI daemon: discover, pair, ping round-trip, clean
      shutdown/join of the async runtime
- [ ] **Verified** — pairs with the stock Android app and pings both ways on a
      real network; reconnect re-verifies the pinned cert; unpair is honored
- [ ] **Measured** — installed closure size and daemon idle wakeups reported and
      inside a declared budget

**Done when:** an unpaired phone and this daemon reach a mutually-trusted,
reconnect-stable TLS channel and exchange pings, verified against the reference
client — and the transport's cost is a number, not a hope.

## Checkpoint 1 — The daily plugins (earn Valent's retirement)
**Goal:** the plugins that make it a daily driver, each chosen by what the author
actually uses in Valent, wired through freedesktop standards. Still daemon-only
(CLI/scriptable); the UI is CP2.

- [ ] **Notifications** — phone notifications mirror to the session's own
      notification daemon (`org.freedesktop.Notifications`), with dismiss and, where
      the phone supports it, quick-reply
- [ ] **Share, both directions** — phone → PC payload transfer into an XDG dir
      Siderita shows; PC → phone as a `share.request` (file / URL / text)
- [ ] **Clipboard** — opt-in text clipboard sync
- [ ] **Battery & connectivity** — phone battery and cell/connectivity status
      exposed for the shell to read
- [ ] **Find-my-phone** — ring the device
- [ ] **Verified** — every plugin exercised against the real device; a source
      file is never removed before its transferred copy is verified (the suite's
      loss-free rule, shared with `siderita-ops`)

**Done when:** the author runs the session with Valent removed from autostart
because Magnetita covers the daily set, proven on a real phone.

## Checkpoint 2 — One suite (stop being an island)
**Goal:** the daemon, the UI and the shell behave as one suite, not a standalone
program with a tray icon.

- [ ] **UI client** — `magnetita-qt` + a Qt/QML host over `celestina-style`:
      device list, pair/unpair with the verification key, transfer progress,
      per-plugin toggles; a thin client of the daemon
- [ ] **Suite IPC/activation** — ratify the daemon↔UI (and shell) convention this
      project forces; this becomes the suite's reference for that contract
- [ ] **Shell surfacing** — connected device + battery in the `celestina-desktop`
      panel (notifications already flow via freedesktop)
- [ ] **Siderita integration** — "Send to phone" in Siderita's context menu via
      `open-with`
- [ ] **One settings source** — paired devices and per-plugin toggles share the
      suite's settings/theming source, not a private store
- [ ] **Media control (MPRIS)** — control phone/desktop playback both ways

**Done when:** pairing, status, transfers and settings are reachable from the
suite's own surfaces in the suite's own visual language, over one IPC contract.

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
