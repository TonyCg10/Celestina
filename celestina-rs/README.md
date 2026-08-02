# celestina-rs

The suite's shared Rust workspace: interface-neutral domain cores that carry no
Qt, QML or Niri types — plus, as the one deliberate exception, Magnetita's
network transport and headless daemon. Presentation lives in each app.

- **Role:** shared domain cores + the phone-link backend (part of the [Celestina suite](../ROADMAP.md))
- **Toolchain:** Rust 2021, pinned 1.97.1 (MSRV floor `rust-version = "1.85"`) · `unsafe_code = "forbid"` (workspace lint)
- **Dependencies:** every core stays free of third-party crates except
  `grafita-core`, which needs `xattr` because reproducing a file's extended
  attributes and POSIX ACLs uses syscalls `std` does not expose, and
  `fluorita-core`, which needs `md5` because the freedesktop thumbnail cache
  keys its entries on that digest; `fluorita-engine` is the one crate that links
  a media stack, `libmpv`, chosen by measurement and approved by the author;
  `magnetita-core` remains UI/IO-free but uses serde for its wire domain, and the
  transport/daemon earn rustls + ring, rcgen and zbus. Each dependency is
  justified inline in its `Cargo.toml`

## Crates

| Crate | Responsibility |
|---|---|
| `celestina-core` | shared generations, cooperative cancellation, atomic durable state-file replacement, percent-encoding, XDG paths |
| `siderita-core` | read side: `EntryId` identity, snapshots, bounded scan executor, view projection, watch state |
| `siderita-ops` | write side: loss-free create/rename/copy/move/trash/restore/purge |
| `siderita-qt` | stable opaque view tokens — the contract toward Qt/QML |
| `celestina-dotfiles-core` | plan-only dotfiles change planning (records conflicts, never mutates) |
| `fluorita-core` | Fluorita's media core: media identity/kind, configured library roots, catalogue with honest unknown metadata, Gallery/Music projections, generation-stamped playback whose state moves only on engine reports, the Qt-compatible freedesktop thumbnail key/validity contract Siderita consumes, and static-artwork versus bounded live-trailer requests |
| `fluorita-engine` | Fluorita's media engine over libmpv, behind a narrow replaceable contract: metadata probing with bounded budgets, freedesktop poster/cover publication, bounded live trailers in a pruned private cache, and playback sessions whose confirmed state comes only from backend reports. Byte-safe on non-UTF-8 names, and never the path an image thumbnail takes |
| `fluorita-qt` | the render seam both media hosts compile: the hand-written `QQuickFramebufferObject` that drives libmpv's render API on Qt's render thread, which CXX-Qt cannot express. No dependencies and no behaviour — it names source files and an include directory for a consuming `build.rs`, so neither application has to copy it or depend on the other's tree |
| `grafita-core` | Grafita's document core: content-only text classification, reversible encodings, a byte- and newline-preserving buffer, splice/undo/redo/savepoint, literal find/replace whose replace-all is one undoable action, measured indentation and go-to-line, a dependency-free lexer that colours eight languages and leaves the rest as plain text, dirty and conflict state, a save that reproduces metadata and refuses rather than destroys, the line-feed projection a text widget edits (with the reconciliation that keeps a widget from rewriting terminators), the bounded, joined worker both hosts run it all on, and the open/edit/save/close session — staleness rules included — that leaves each host only its Qt marshalling |
| `celestina-shell-core` | what every Celestina shell helper shares with its Qt host: bounded line framing that recovers after a hostile line, the one serialized writer a multi-threaded helper emits whole frames through, the provider envelope (identity, bounds, generations, "same value is not news") and the typed, bounded command vocabulary with the refusals that answer an unreadable request |
| `magnetita-core` | KDE Connect protocol domain: packets, identity, v8 timestamp-aware pairing with typed invalidity, plugin bodies, typed MPRIS actions and playback-progress classification — pure, offline-tested |
| `magnetita-net` | phone-link transport: UDP discovery, identity-bound TCP+TLS, stable TOFU certificate pin versus temporary SPKI+timestamp code, bounded certificate-bound payload transfer, atomic trust store |
| `magnetitad` | headless phone-link daemon: explicit local pairing acceptance and durable revocation, admitted/expiring unknown links, sshfs mount, lossless incoming files, a bounded/cancelable joined desktop-MPRIS worker, bounded payload workers whose stale results cannot publish after `Forget`, temporary `verificationKey`, `org.celestina.Devices1` |

Workspace-specific dependency, safety and verification rules are recorded in
[`AGENTS.md`](AGENTS.md), layered on the monorepo contract.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Apps use `path` dependencies during development; a release consumes pinned
versions of these crates.

See [ROADMAP.md](ROADMAP.md) for status and checkpoints.
