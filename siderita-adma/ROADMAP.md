# Siderita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the file
> manager only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is
> not "verified": real-Wayland acceptance is tracked as its own goal.

## Overview

**Purpose.** The suite's independent file manager — navigate, organize and
retrieve local and removable files, integrated through freedesktop standards. No
editor, viewer, player, panel or dotfiles manager inside it; those are separate
projects reached through desktop standards.

**Current state.** The earlier C++/Qt prototype was removed; the Rust host of
`qml/i1` is the only implementation. It consumes the `celestina-rs` domain crates
(`celestina-core`, `siderita-core`, `siderita-qt`) and now renders from the shared
`celestina-style` module (tokens + glass). The functional slice is deliberately
read-only (Iteration 1): HOME/path navigation, filter, sort, hidden toggle, stable
selection and truthful states, with a bounded scan worker that publishes on the Qt
thread and rejects stale results. The core preserves Unix and non-UTF-8 identity,
uses generations and opaque tokens, and provides cancellation/join; the 30
workspace tests plus the host's own bookmark/places unit tests pass. The UI has
grown past the minimal slice — multi-selection, context menus, sidebar places
(XDG) and bookmarks — but stays read-only toward the user's files: the only writes
are Siderita's own bookmark config under `~/.config`. Installation staging,
watcher, `file://`, a native role-based model, UI tests and real-Wayland
blur/frame numbers are still open, so Qt/QML stays a **provisional** first
iteration.

**Key decisions.** Siderita keeps its own roadmap and release; the Rust cores live
in a separate workspace so each domain is testable without a toolkit; C++ is
limited to the CXX-Qt bridge; internal glass lives in QML (bounded capture +
`MultiEffect`, translucent fallback); icons use freedesktop names with a minimal
SVG fallback; Qt dependencies stay under an allowlist; the visible name is never
identity (homonyms, rename and non-UTF-8 names are preserved); a source is never
deleted before its destination is verified; integration is via XDG/freedesktop;
shared style comes by contract, never by importing another source tree.

## Checkpoint 0 — Truthful, measured read-only slice (I1)
**Goal:** a staged install opens HOME, a path, or a local URI in a modern
read-only view; a context menu demonstrates real in-scene glass; and the
resource report ratifies or rejects Qt/QML with data.

### Implemented
- [x] `celestina-core` / `siderita-core` / `siderita-qt` neutral up to the Qt edge (PathBuf/OsString, EntryId, generations, opaque tokens)
- [x] Bounded scan executor with cancellation and deterministic shutdown
- [x] Qt Quick/QML UI + minimal CXX-Qt adapter (provisional for I1)
- [x] Content layer separated from overlay + a shareable `GlassSurface` (bounded one-shot capture, no continuous work when the menu is closed)
- [x] HOME / path navigation (back / forward / up / home / refresh) incl. mouse side buttons
- [x] Filter with 120 ms debounce; sort by name / size / date / kind, both directions, folders first; stable selection across re-sort; hidden toggle
- [x] Segmented breadcrumb + list/grid view toggle
- [x] Adjustable item size — a status-bar zoom slider (up to 190 %) scales rows/cells, glyphs, icons and labels in both views (session-local)
- [x] Multi-selection — plain / Ctrl / Shift click, a drag-marquee zone, right-click-selects and select-all; the status line shows the count; token-keyed so it survives sort/filter and clears on navigation
- [x] Per-item and empty-space (folder) context menus with the glass background
- [x] Sidebar places — Inicio plus the standard XDG user folders (Escritorio, Documentos, Descargas, Música, Imágenes, Vídeos), resolved from `user-dirs.dirs` and shown only when they exist
- [x] Sidebar bookmarks — add from a folder's context menu or by dragging it onto the sidebar, rename and remove, navigate; persisted across restarts to `~/.config/siderita/bookmarks.tsv`
- [x] Header moved into the content box as independent glass pills (breadcrumb + search) that fade to glass on scroll; controls consolidated into a bottom bar; sidebar bottom info box
- [x] Host-side Rust for `entry_path`, the bookmark store and the XDG places resolver, with unit tests (bookmark round-trip + sanitization, `user-dirs.dirs` parsing)
- [x] Truthful loading / empty / error / degraded-watch states
- [x] Freedesktop theme icons with minimal embedded SVG fallbacks
- [x] Dependency inventory + size / memory / CPU / threads baselines (offscreen)

### To finish CP0
- [ ] Fix truthful-state gap: a failed navigation via back/forward/up/home/activate must not leave the path pointing at an unreadable directory while the list still shows the previous one — route all navigation through commit-on-success (only the typed-path bar does this today)
- [ ] Local `file://` URI handling
- [ ] Watcher wired to `WatchState` (invalidate + rescan wins)
- [ ] Replace `QStringList` with a native role-based `QAbstractListModel`, dropping the per-delegate token/kind/subtitle invokables and the `viewRevision` workaround
- [ ] Give the grid view keyboard navigation (only the list handles keys today)
- [ ] Staged install with an allowlist (Basic + only the plugins actually used)
- [ ] Real-Wayland validation: keyboard, contrast, animations, themed icons; blur on/off frame p95 ≤ 16.7 ms, measured three times
- [ ] Ratify Qt/QML for the suite, or reopen the frontend decision, from the data

### Provisional budget

| Metric | Limit | Current cut |
|---|---:|---:|
| Stripped isolated binary | 20 MiB | 1.62 MiB; staging pending |
| First install closure (Qt) | 250 MiB | pending |
| HOME mean PSS, 60 s | 120 MiB | 40.52 MiB, one offscreen run |
| HOME one-core CPU, 60 s | 1 % | 0.000 %, one offscreen run |
| 10k-entry fixture mean PSS | 250 MiB | 46.47 MiB, one offscreen run |
| Menu blur frame p95 | 16.7 ms | pending on real Wayland |

Measured 2026-07-20 on the offscreen/software backend, one run per scenario — not
a substitute for three runs in a real Wayland session; GPU memory and the
first-install Qt closure are not yet counted. Limits may tighten with evidence;
loosening one needs an explicit decision with a demonstrated benefit. "The whole
suite's usage is marginal" is not an accepted justification for a regression.

**Dependencies (allowlist).** The runtime pins CXX 1.0.176 and CXX-Qt 0.9.1 with
only `qt_gui`, `qt_qml` and `qt_quickcontrols` in `cxx-qt-lib`. The Qt allowlist
starts from Core/Gui/Qml/Quick/QuickControls2 + `QtQuick.Effects` and excludes
Concurrent, WebEngine, Multimedia and KDE/GNOME frameworks; nothing is added
without a measured need. The UI needs Qt 6.8+ for `Popup.Item`.

**Done when:** build/test/install start from a declared environment with no
sibling paths in a release; core tests cover generations, cancellation, bursts,
hardlinks, non-UTF-8 names and tokens; HOME/path/URI show loading, snapshot or
recoverable error correctly; a scan for A never publishes after navigating to B;
lost watch degrades visibly; homonyms, hardlinks and non-UTF-8 names never
collapse identity; the menu blurs app content, not a copy of itself; the
dependency inventory holds no unjustified out-of-allowlist entries; and every
budget number is attached to the measured artifact, or Qt/QML is marked
unratified.

## Checkpoint 1 — Loss-free operations (S1)
**Goal:** create, rename, copy, move and send-to-Trash on disposable fixtures
only, with no silent data loss.

- [ ] Wire the write-side domain from `celestina-rs` CP1 into the app
- [ ] Preflight + conflict resolution + cancellation + per-item results
- [ ] Freedesktop Trash support
- [ ] Guarantee: a source is never removed after a partial copy or without revalidation

## Checkpoint 2 — Interoperable daily manager (S2)
**Goal:** a manager good enough for daily use, integrated through standards.

- [ ] XDG Trash restore, cross-filesystem moves, and volume handling
- [ ] Safe open-with/handlers, drag-and-drop, and `org.freedesktop.FileManager1`
- [ ] Full accessibility and daily-use resource budgets
- [x] Consume the shared CelestinaStyle tokens, glass and fallback icons — the theme/glass now live canonically in `../celestina-style` and are compiled into this module; Siderita's private copies were removed (verified: builds + offscreen run clean). Installed-release form is tracked in CelestinaStyle CP0.

## Non-goals

No cloud/network, global indexer, archive VFS, plugins, IDE, terminal or suite
daemon, and no features of other applications, before the declared local manager
is complete.
