# Magnetita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It adds
constraints for `magnetita/` and its registered crates; it cannot relax the root
or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `magnetita/` is a thin Qt/QML client plus packaging. Domain, protocol,
  transport, and service belong to `magnetita-core`, `magnetita-net`, and
  `magnetitad` in `celestina-rs`.
- The UI requests actions and reflects only snapshots confirmed by
  `org.celestina.Devices1`; it does not maintain optimistic parallel truth.
- Blocking D-Bus never runs on the Qt thread. An owned bounded worker orders
  actions; reads/watchers coalesce bursts and apply the latest snapshot. Every
  touched lifecycle gains deterministic shutdown.
- Evolve `org.celestina.Devices1` compatibly: preserve methods and extend
  `a{sv}` additively.
- Preserve measured KDE Connect invariants: the phone drives pairing; the side
  initiating the payload connection is the TLS server; payloads use 1739–1764;
  phone-to-PC clipboard is manual because of the observed Android restriction.
- Treat network input, names, certificates, sizes, and payloads as hostile.
  Pairing needs explicit acceptance; `Forget` is a durable barrier and later
  results from the revoked source cannot publish.

## Service and deployment

Build and verify never stop `magnetitad`. `scripts/complete-production.sh` is
mandatory when closing a bug or milestone; it may stop an already-active service
once, copy the verified daemon, and restart it. It never enables an inactive
service. When `magnetita-core` changes, also run
`celestina/scripts/complete-production.sh` because the shell consumes its phone
projection; this updates the on-disk shell bundle but does not activate it. Do
not delete trust/configuration or force re-pairing for verification.

## Local verification

- `magnetita/scripts/build-production.sh`
- `magnetita/scripts/verify-production.sh`
- `magnetita/scripts/status-production.sh`
- Producer/consumer tests when D-Bus or protocol changes

Phone, pairing, mount, transfer, hardware, and Wayland tests belong in
`VALIDATION.md`; loopback does not replace them or keep implementation open.
Review network security, worker ownership, QObject affinity, and consumer
compatibility.
