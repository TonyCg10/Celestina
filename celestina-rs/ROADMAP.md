# celestina-rs roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the shared
> Rust cores only. Checklist legend: `[x]` done · `[ ]` planned.

## Overview

**Purpose.** The suite's shared domain foundation: reusable, interface-neutral
Rust logic consumed by every app. Presentation lives in each app; Qt, QML, Niri
and network IO never enter here.

**Current state.** Four crates compile with `fmt`, Clippy and 30 tests green on
Rust 1.85.1, with no third-party dependencies and `#![forbid(unsafe_code)]`.
Everything is read-only — there is no write or apply API yet. `siderita-core` and
`siderita-qt` are consumed live by Siderita; `celestina-dotfiles-core` only
produces plans.

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
- [x] fmt, Clippy `-D warnings`, 30 tests green; `#![forbid(unsafe_code)]`
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

- [ ] create / rename / copy / move / trash primitives with preflight, conflict detection, cancellation, and per-item results
- [ ] never remove a source before its destination is verified; explicit cross-filesystem revalidation
- [ ] `celestina-dotfiles-core` — a transactional apply API (today it only produces plans), reversible where possible and separate from planning

**Done when:** every operation reports per-item success/failure and no code path
can lose data.

## Checkpoint 2 — Shared cores for more apps (CORE-2)
**Goal:** extract genuinely shared domain as new apps arrive, without leaking one
app's internals into another.

- [ ] shared crates for config, an IPC/activation convention, XDG/MIME, and handler helpers
- [ ] each app's own domain stays in its own crate; `siderita-qt` remains the pattern for per-app view adapters

## Non-goals

No Qt/QML or Qt/Niri wire types in the pure cores; no IO in `siderita-qt`; no
networking; no applying system changes from planning crates without an explicit,
tested apply API; no cross-app internals; no third-party dependencies without a
demonstrated, measured need.
