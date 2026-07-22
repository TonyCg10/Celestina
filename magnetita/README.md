# Magnetita

The suite's phone link: a small, native service that connects the desktop to a
phone over the local network — notification mirroring, file share both
directions, clipboard, battery and find-my-phone — so the whole Celestina
session syncs through one first-party daemon instead of a foreign desktop client.

It speaks the **KDE Connect network protocol** on the wire, so it pairs with the
stock (FOSS) KDE Connect Android app and interoperates with other KDE Connect
desktops. What it replaces is the third-party *desktop* daemon (Valent /
`kdeconnectd` and their GTK/KDE closures) — not the phone app.

- **Role:** phone link / device sync (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust daemon · `rustls` (TLS 1.2+) · Qt Quick/QML via CXX-Qt (UI) · GPL-3.0-or-later
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores · [celestina-style](../celestina-style/) tokens + glass
- **Speaks:** the KDE Connect protocol (interoperates with the KDE Connect Android app and other KDE Connect desktops)

> **Status: design stage.** This directory holds the roadmap and contracts only;
> there is no implementation yet. Nothing below is verified — see
> [ROADMAP.md](ROADMAP.md) for the checkpoint ladder and what "done" means.

## Why a first-party daemon

The phone-link feature is a proven daily need (currently served by Valent), and
its protocol is an open, multi-implementation de-facto standard — the same kind
of standard the suite already leans on for Trash, URIs and `.desktop` entries.
So a native daemon is *standards-interop*, not reinvention: it lets the session
own its phone integration end-to-end (one closure, one settings source, one
visual language, notifications through the session's own notification daemon)
while staying a good desktop citizen on the wire.

Per suite discipline, Valent stays composed in autostart until Magnetita's
transport is **verified** against a real device — the same way
`celestina-desktop` keeps Noctalia until its own pieces are proven.

## Shape

Magnetita is primarily a **headless daemon** with an **optional UI client** —
unlike Siderita, which is a windowed app. The daemon runs as a systemd *user*
service, holds the device connections, and routes to the rest of the desktop
through freedesktop standards (phone notifications → `org.freedesktop.Notifications`,
inbound files → an XDG folder Siderita shows). The UI is a thin client for
pairing, device status and transfers; the service works without it.

## Layout (planned)

| Path | Responsibility |
|---|---|
| `../celestina-rs/crates/magnetita-core` | protocol domain: `NetworkPacket`, identity, capabilities, pairing state machine, packet serde — pure, no I/O, no Qt |
| `../celestina-rs/crates/magnetita-net` | service engine: discovery, TCP+TLS transport (TOFU cert pinning), connection lifecycle, plugins |
| `../celestina-rs/crates/magnetita-qt` | CXX-Qt view contract for the UI (device model, pairing, transfer progress) |
| `src/` (`magnetitad`) | the headless daemon binary and its systemd user unit |
| `src/` (UI host), `qml/` | the optional Qt/QML client, consuming `celestina-style` |
| `../celestina-style/` | shared theme, glass and icons (consumed) |
| `scripts/` | run and measurement scripts |

## Standards & interop

- **On the wire:** the KDE Connect protocol — line-delimited JSON `NetworkPacket`s
  over TLS 1.2+, UDP identity broadcast on port 1716, trust-on-first-use cert
  pinning. Interoperability with the reference Android client is a **contract**,
  not a nicety.
- **To the rest of the desktop:** freedesktop only — notifications, XDG dirs,
  MIME/`open-with`, `.desktop` entries. No private glue.

See [ROADMAP.md](ROADMAP.md) for status, the checkpoint ladder, the dependency
budget and the design decisions.
