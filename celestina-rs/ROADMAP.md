# celestina-rs roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the shared
> Rust cores only. Checklist legend: `[x]` done · `[ ]` planned.

## Overview

**Purpose.** The suite's shared domain foundation: reusable, interface-neutral
Rust logic consumed by every app. Presentation lives in each app; Qt, QML and
Niri types never enter here. Network IO never enters the *cores* — the
magnetita subsystem (`magnetita-net` + `magnetitad`) is the workspace's one
deliberate, contained exception.

**Current state.** The workspace compiles with `fmt`, Clippy and its test suite
green on Rust 1.97.1, `unsafe_code = "forbid"` throughout. The cores carry no
third-party dependencies except `grafita-core`, whose metadata-preserving save
needs extended-attribute syscalls `std` does not expose, and `fluorita-core`,
whose thumbnail key is an MD5 the freedesktop cache mandates; `fluorita-engine`
is the workspace's only link against a media stack (libmpv, measured and
author-approved); `magnetita-core`
is pure with respect to UI/IO but uses serde for the wire model. The read side
(`siderita-core`, `siderita-qt`) and the write side (`siderita-ops`, loss-free
file operations) are consumed live by Siderita; `grafita-core` backs two host
adapters — Grafita's own window and Siderita's embedded modal — while
`fluorita-core` backs `fluorita-engine`, which decodes real media for two hosts
— Fluorita's window and Siderita's embedded modal — over the render seam
`fluorita-qt` owns;
`celestina-dotfiles-core` only produces plans;
the Magnetita trio backs the shipped phone link. Its earlier release was
verified against the real phone; the current pairing-v8, admission, revocation,
post-transfer publication and MPRIS hardening is offline/loopback-verified and
still awaits a fresh real phone/Wayland acceptance pass.

**Key decisions.** The core family lives in its own workspace so each domain is
testable without a toolkit; apps use `path` deps in development but pin versions
for a release; identity is never the visible name (parent device+inode + raw
`OsString` preserves non-UTF-8 and distinguishes hardlinks); staleness is rejected
by generation; the CXX-Qt QObject is an optional adapter feature, not part of the
pure cores.

## Checkpoint 0 — Versioned, read-only cores (CORE-0)
**Goal:** apps consume pinned, versioned crates (never `path` deps) for any
release; `siderita-qt` is the stable contract toward Qt/QML; the domain is proven
read-only.

- [x] `celestina-core` — generations that never wrap silently + cooperative cancellation (`Release`/`Acquire`)
- [x] `siderita-core` — `EntryId` identity (parent device+inode + raw `OsString`) that preserves non-UTF-8 names and distinguishes hardlinks
- [x] `siderita-core` — bounded scan executor with cancellation + join; non-symlink-following scan; non-mutating view projection; pure navigation history; `WatchState` that separates health from freshness
- [x] `siderita-qt` — stable opaque view tokens that survive filter/sort and never key on the display name
- [x] `celestina-dotfiles-core` — plan-only dotfiles (records conflicts, never creates/replaces/removes)
- [x] fmt, Clippy `-D warnings`, workspace tests green; `unsafe_code = "forbid"`
- [ ] Freeze the public API of all four crates and document the stability/compatibility promise
- [ ] Decide the crate versioning + release policy (bump the family together when they share a contract)
- [ ] Add the CXX-Qt QObject as an optional adapter feature of `siderita-qt` once Qt is in the declared environment (no domain moves to C++)
- [ ] Executor: cancel the **running** scan on re-enqueue, not just the pending one, so large/slow directories don't burn work
- [ ] Expand tests as new consumers of the contract appear

**Done when:** apps depend on pinned versions with no sibling paths in a release;
no core exposes Qt or Niri types or does network IO; all gates stay green and
`forbid(unsafe)` holds.

## Checkpoint 1 — Loss-free operations domain (CORE-1)
**Goal:** the write-side domain that Siderita CP1 stands on, with no silent data
loss.

- [x] create / rename / copy / move / trash primitives with preflight, conflict detection, cancellation, and per-item results — shipped as `siderita-ops`, consumed live by Siderita v1.0 (its CP1 ratified the behavior on disposable fixtures)
- [x] never remove a source before its destination is verified; explicit cross-filesystem revalidation — `relocate.rs` copies, verifies kind+length, and only then removes; cancel keeps the source and rolls back the partial destination
- [ ] `celestina-dotfiles-core` — a transactional apply API (today it only produces plans), reversible where possible and separate from planning

**Done when:** every operation reports per-item success/failure and no code path
can lose data.

## Checkpoint 2 — Shared cores for more apps (CORE-2)
**Goal:** extract genuinely shared domain as new apps arrive, without leaking one
app's internals into another.

- [x] `celestina-shell-core` — what the shell's helpers share with their Qt
      host, extracted from the Niri adapter when a second helper needed exactly
      it, never invented ahead of one: bounded line framing that discards a
      hostile line through its newline and recovers, one serialized writer so
      two producing threads cannot interleave a frame, the provider envelope
      (bounded identity, per-provider payload caps, generations, and "the same
      value is not news" so an idle panel stays idle) and the command
      vocabulary whose refusals carry the request id whenever one can be
      recovered. Its first consumer is the Niri adapter, which lost its private
      copies in the same change; the aggregate provider helper is the second
- [x] `grafita-core` — the document core behind both of Grafita's surfaces:
      classification by content alone (never by extension or MIME), the
      reversible encodings UTF-8/UTF-8 BOM/UTF-16 LE·BE BOM, a buffer whose
      lines keep their own terminators so an untouched open/save is
      byte-identical, splice/undo/redo/savepoint with typed position refusals,
      and a save that re-verifies the resolved target, reproduces mode,
      ownership and extended attributes onto a sibling temporary, and renames
      only after everything is durable. Verified against real files, including
      symlink, changed-underneath, retargeted, deleted-target and interrupted
      save cases that all leave the original file intact. It embeds no runtime:
      `open` and `save` are plain blocking calls a host runs on its own worker,
      stamped with a generation and a revision so a stale reply cannot replace
      newer state. It also owns the two pieces a toolkit host would otherwise
      have to reinvent: a bounded worker that supersedes stale probes, never
      drops a promised save and joins deterministically on drop, and the
      line-feed projection a text widget edits — reconciled back into a single
      splice, so a widget that only knows line feeds can never rewrite a CRLF
      file's terminators. On top of both sits `session`: the open/edit/save/close
      state machine with its generation and revision staleness rules, pure and
      synchronously testable, which is what lets Grafita's standalone app and
      Siderita's embedded modal share behaviour while wording it differently
- [x] `fluorita-core` — the media core behind both player surfaces: kind
      classification that covers everything Siderita thumbnails, configured
      library roots that refuse to nest, a catalogue whose reconciliation marks
      a file missing instead of deleting anything, deterministic Gallery and
      Music projections with an explicit unknown bucket, playback whose
      confirmed state moves only on generation-stamped engine reports while a
      click stays a pending request, and static freedesktop artwork kept
      type-distinct from bounded, cancelable live trailers. The thumbnail key
      is frozen against golden vectors measured from Qt 6 itself, so Siderita
      reads what Fluorita writes without changing a line. Verified by unit
      tests covering the golden vectors, a non-UTF-8 name, staleness and
      cancellation; `celestina-core::percent::encode_qt_path` was added beside
      the existing Trash codec because the two preserved sets genuinely differ
- [x] `fluorita-engine` — the same media truth, now coming from a real decoder:
      libmpv behind a narrow `MediaEngine`/`EngineSession` contract, bounded
      metadata probing, freedesktop poster and embedded-cover publication that
      lands byte-for-byte on the path `fluorita-core` computes (owner-only, via
      a staging directory inside the cache root so the last step is a rename),
      playback sessions whose confirmed state moves only on backend reports and
      whose seeks are only complete when the backend restarts, and a joinable
      worker whose shutdown is a message rather than a dropped sender — the
      failure its own test caught. Bounded live trailers land in Fluorita's own
      pruned cache, never as a freedesktop entry, and are verified by decoding
      the encode back before it is published. Non-UTF-8 filenames travel as `fd://` rather
      than through a lossy conversion that would open another file. It also
      walks the configured library roots — bounded by files, depth, deadline and
      cancellation, not following symlinks, opening nothing — so a catalogue
      scan costs `stat` calls rather than decodes. Verified against real libmpv
      on tiny synthetic fixtures; the Qt Quick render path
      is not built yet
- [x] `fluorita-qt` — the render seam, extracted when a second host needed it
      and not before: Fluorita's window and Siderita's embedded modal both need
      the same `QQuickFramebufferObject` over libmpv's render API, CXX-Qt 0.9
      cannot express it, and hand-written C++ had always lived in the app that
      needed it. Copying it would be duplication and symlinking Fluorita's
      `cpp/` would make one application depend on another's tree, so the seam
      became a crate beside `siderita-qt`. It carries no behaviour and no
      dependencies — it names the sources and the include directory a consuming
      `build.rs` compiles — and its tests assert that those names are true
- [ ] shared crates for config, an IPC/activation convention, XDG/MIME, and handler helpers
- [ ] each app's own domain stays in its own crate; `siderita-qt` remains the pattern for per-app view adapters

## Non-goals

No Qt/QML or Qt/Niri wire types in the pure cores; no IO in `siderita-qt`; no
networking in the domain cores (the `magnetita-net`/`magnetitad` pair is the
suite's deliberate, contained exception); no applying system changes from
planning crates without an explicit, tested apply API; no cross-app internals;
no third-party dependencies without a demonstrated, measured need.
