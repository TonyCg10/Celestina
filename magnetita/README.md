# Magnetita

The suite's phone link: a small, native service that connects the desktop to a
phone over the local network — and, first among everything else, hands that
phone to **Siderita**, mounted and always there, so the file manager browses it
like any other device. Notification mirroring, file share both directions,
clipboard and battery come after.

It speaks the **KDE Connect network protocol** on the wire, so it pairs with the
stock (FOSS) KDE Connect Android app and interoperates with other KDE Connect
desktops. What it replaces is the third-party *desktop* daemon (Valent /
`kdeconnectd` and their GTK/KDE closures) — not the phone app.

Magnetita is the suite's **first cross-app integration** and its **first
networked app**: everything before it stayed on one machine.

- **Role:** phone link / device sync (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust daemon · `rustls` (TLS 1.2+) · Qt Quick/QML via CXX-Qt (UI) · GPL-3.0-or-later
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores · [celestina-style](../celestina-style/) tokens + glass
- **Speaks:** the KDE Connect protocol (interoperates with the KDE Connect Android app and other KDE Connect desktops)

> **Status: started.** `magnetita-core` has begun — the packet envelope and the
> identity packet, as pure Rust types that (de)serialize the wire format, unit-
> tested without a socket (8 tests). The trusted channel and the Siderita mount
> are next. Everything past the core is unverified until it runs against a real
> phone; see [ROADMAP.md](ROADMAP.md).

## The phone in Siderita — the first cross-app experience

The reason Magnetita is built before the suite's other new apps: it is the first
one that has to integrate with another. Two layers, decided deliberately because
this sets the pattern every later integration copies:

- **The data is a real mount.** Magnetita mounts the phone's storage over the KDE
  Connect SFTP plugin (sshfs + FUSE) at a stable, owned path under
  `$XDG_RUNTIME_DIR/magnetita/`, and keeps it mounted while the phone is
  connected and paired — *always mounted*, not on-demand. Browsing the phone in
  Siderita is then **ordinary filesystem navigation**; Siderita's views need no
  special case for it.
- **The sidebar entry and its state are a versioned contract.** Magnetita serves
  `org.celestina.Devices1` on the session bus — the connected phones, each with
  its id, name, type, battery, connection state and mount path — and Siderita
  *consumes* it to draw the device and open its mount. It mirrors how Siderita
  already *serves* `org.freedesktop.FileManager1` and the file-chooser portal,
  only now it is on the reading end. State stays truthful — *connecting →
  mounting → mounted at `<path>`* — because a click on the device is a request,
  never a proof.

The filesystem carries the bytes; the contract carries what the filesystem
cannot — a name, a battery, a connection. This is the suite's **first internal
contract**, D-Bus-shaped like the freedesktop ones on purpose.

## Why a first-party daemon

The phone-link feature is a proven daily need (currently served by Valent), and
its protocol is an open, multi-implementation de-facto standard — the same kind
of standard the suite already leans on for Trash, URIs and `.desktop` entries.
So a native daemon is *standards-interop*, not reinvention: it lets the session
own its phone integration end-to-end (one closure, one settings source, one
visual language, notifications through the session's own notification daemon)
while staying a good desktop citizen on the wire.

Per suite discipline, Valent stays composed in autostart until Magnetita's
transport is **verified** against a real device — the same way `celestina` keeps
Noctalia until its own pieces are proven.

## Shape

Two parts. A **background service** holds the device links, keeps the phone
mounted and serves `org.celestina.Devices1`, so *connected and mounted* stays
true whether or not a window is open — Siderita sees the phone even with
Magnetita's UI closed. And a **standalone app**, like Valent's window and a
first-class surface, not an afterthought: it is where you

- pair and unpair a device (with a shown verification key),
- read a **connection log** that says *why* a device will not pair or connect —
  discovery, the TLS handshake, a pairing timeout, an unpaired or unreachable
  peer — a deliberate answer to KDE Connect's and Valent's worst day: a phone
  that silently will not connect, with nowhere to look,
- and set the options that are not Siderita's — per device, per plugin.

The service routes the rest into the desktop over freedesktop standards (phone
notifications → `org.freedesktop.Notifications`, shared-in files → an XDG folder
Siderita shows).

## Layout (planned)

| Path | Responsibility |
|---|---|
| `../celestina-rs/crates/magnetita-core` | protocol domain: `NetworkPacket`, identity, capabilities, pairing state machine, plugin bodies — pure, no I/O, no Qt (**started**) |
| `../celestina-rs/crates/magnetita-net` | service engine: discovery, TCP+TLS transport (TOFU cert pinning), connection lifecycle, plugins |
| `../celestina-rs/crates/magnetita-qt` | CXX-Qt view contract for the UI (device model, pairing, transfer progress) |
| `src/` (`magnetitad`) | the headless daemon: device links, the sshfs mount, the `org.celestina.Devices1` service, and its systemd user unit |
| `src/` (UI host), `qml/` | the optional Qt/QML client, consuming `celestina-style` |
| `../celestina-style/` | shared theme, glass and icons (consumed) |
| `scripts/` | run and measurement scripts |

## Standards & interop

- **On the wire:** the KDE Connect protocol — line-delimited JSON `NetworkPacket`s
  over TLS 1.2+, UDP identity broadcast on port 1716, trust-on-first-use cert
  pinning. Interoperability with the reference Android client is a **contract**,
  not a nicety.
- **To Siderita:** the phone's storage as a real sshfs mount, and
  `org.celestina.Devices1` — the suite's first internal contract — for the device
  and its state.
- **To the rest of the desktop:** freedesktop only — notifications, XDG dirs,
  MIME/`open-with`, `.desktop` entries. No private glue.

## Networked, so security-first

The suite's "no network" was about the cloud and internet indexing, not about
talking to your own phone on the Wi-Fi. Magnetita is LAN-only and peer-to-peer:
TLS on every link, trust only by **explicit pairing** with a shown verification
key, never on a guess, and the identity that travels in the clear carries no
secret.

See [ROADMAP.md](ROADMAP.md) for status, the checkpoint ladder, the dependency
budget and the design decisions.
