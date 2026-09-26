# celestina-rs — local workspace contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
constraints for `celestina-rs/`; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Content activation](../docs/contracts/content-activation.md) when Grafita or
  Fluorita changes

## Local boundary

- Domain crates know nothing about Qt, QML, Niri, or applications. Dependencies
  point from adapters and IO toward core, never in reverse.
- `magnetita-net` (the KDE Connect wire), `magnetita-link` (QUIC on tokio) and
  `magnetitad` are the deliberate transport and service layers,
  `magnetita-mobile` is the phone's UniFFI export and `magnetita-peer` its
  headless binary; `magnetita-core` and `magnetita-proto` remain pure domain.
- `celestina-shell-core` is the shell helpers' domain; its `journal` module is
  its one declared file-writing exception.
- `celestina-core` owns the suite's shared file and path rules: `file://` URIs,
  the runtime and private directories, private-state and media writes, bounded
  small-file reads and `.desktop` reading and scanning. Use those owners
  instead of writing another copy.
- `fluorita-engine` owns multimedia IO and decoding behind the narrow
  `fluorita-core` contract. `fluorita-qt` only owns the shared C++ rendering seam
  CXX-Qt cannot express; it contains no domain or application composition.
- `siderita-qt` only exposes opaque view contracts. File rules belong in
  `siderita-core` and `siderita-ops`.
- Add a dependency only to the layer that needs it and justify it next to the
  manifest. Treat network data, declared sizes, names, and paths as hostile and
  bounded.
- Destructive or move operations preserve the source until the destination is
  verified. Workers, cancellation, and shutdown are bounded and deterministic.
- Every new behavior lands with tests in the same change. Do not create a
  speculative shared core without a real owner and contract.

## Local verification

- `celestina-rs/scripts/build-production.sh`
- `celestina-rs/scripts/verify-production.sh`
- `celestina-rs/scripts/status-production.sh`
- Affected protocol, filesystem, or backend tests selected by the standard
  script

This workspace is nondeployable and has no `deploy-production.sh`. Phone,
compositor, and interaction tests belong to the consuming product's
`VALIDATION.md`. Apply expert Rust/C++/Qt/QML judgment to ownership, threads,
FFI, registration, and consumers even when the main change is pure Rust.
