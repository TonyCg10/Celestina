# Fluorita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Fluorita constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [Content activation](../docs/contracts/content-activation.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `fluorita-core` owns media identity/type, catalogue/projections,
  capabilities, generations, and confirmed state; it contains no Qt or decode.
- `fluorita-engine` owns scan, watch, metadata, persistence, artwork, trailers,
  and playback behind bounded contracts.
- `fluorita-qt` is the shared C++/Qt Quick video-rendering seam. Manual C++
  exists only for concrete CXX-Qt limitations.
- `src/` adapts to CXX-Qt; `qml/` composes Gallery, Music, and player. Siderita
  owns another adapter/modal; neither imports the other's QML or duplicates
  core/engine rules.
- Gallery and Music project configured local roots; they are not file managers
  and do not authorize scanning the whole system.

## Resources, lifecycle, and security

- A freedesktop thumbnail is static PNG. A trailer is live, bounded,
  cancellable, and never published as a standard thumbnail.
- Navigation does not initialize the heavy backend. Decode/playback begins only
  on explicit request and its session closes deterministically.
- Scan, metadata, extraction, and playback never block the GUI thread. Every job
  validates generation and identity before publishing; discard stale responses.
- Treat names, tags, dimensions, duration, and content as hostile. Byte, pixel,
  time, depth, and count limits precede allocation or decode.
- Removing a catalogue item never deletes its source file. Requested state is
  not presented as confirmed until the engine reports it.

## Two surfaces

- In Siderita, Space on image/video/audio opens minimal Fluorita; double-click
  or Enter opens the full app on that item.
- The embedded surface exposes only content, honest state, and supported
  transport. Gallery, Music, sources, and settings belong to standalone.
- The modal blocks the folder, contains/restores focus, and closes/cancels its
  session on exit. Normal navigation consumes static artwork only.

## Local verification

- `fluorita/scripts/build-production.sh`
- `fluorita/scripts/verify-production.sh`
- `fluorita/scripts/status-production.sh`

Verification uses the canonical release artifact without replacing the
installed binary or registering MIME. Closing a bug or milestone runs
`fluorita/scripts/complete-production.sh`; when a shared Fluorita crate changes,
it also runs `siderita/scripts/complete-production.sh` so both installed
consumers carry the verified bytes. A docs-only change or audit does not deploy.
Report any effective handler change. Playback, frame pacing, tearing, focus,
and real visual perception belong in `VALIDATION.md`. Review ownership, thread
affinity, libmpv/render lifecycle, QML registration, and both engine consumers.
