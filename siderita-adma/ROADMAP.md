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
(`celestina-core`, `siderita-core`, `siderita-ops`, `siderita-qt`) and now renders
from the shared `celestina-style` module (tokens + glass). The read side is a
bounded scan worker that publishes on the Qt thread and rejects stale results:
HOME/path navigation, filter, sort, hidden toggle, stable selection and truthful
states. The core preserves Unix and non-UTF-8 identity, uses generations and
opaque tokens, and provides cancellation/join; the workspace tests (including
`siderita-ops`' loss-free-operation coverage) plus the host's own bookmark/places
unit tests pass. The UI has grown past the minimal slice — multi-selection,
context menus, sidebar places (XDG), bookmarks and tabs — and CP1's loss-free
write verbs (new folder/file, rename, copy, move, send-to-Trash) are now wired
from the `siderita-ops` crate, each refusing to overwrite and never removing a
source before its destination is verified. Installation staging,
watcher, `file://`, a native role-based model, UI tests and real-Wayland
blur/frame numbers are still open, so Qt/QML stays a **provisional** first
iteration. The arc from here is deliberate: **operations** (CP1) make it a
manager, **interoperation** (CP2) make it a good desktop citizen, and **comfort**
(CP3) adds what a daily manager is expected to have — each feature earned by a
demonstrated need, never by parity.

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
- [x] Tabs — open a folder in a new tab (middle-click or its context menu), shown as a scrollable strip of isolated glass pills below the breadcrumb/search that fade to glass on scroll; each tab is an independent navigation context (its own history, scan worker and selection), closable via × / middle-click, with `Ctrl+T` / `Ctrl+W` / `Ctrl+Tab`
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
only, with no silent data loss — the step that turns the read-only viewer into a
manager. Opening files through their handler lands here too; deeper handler
management is CP2.

The write-side domain lives in the `siderita-ops` crate — all six verbs, pure and
toolkit-free, 34 tests. Every verb refuses to overwrite and never removes a
source before its destination is verified.

- [x] Wire the write-side domain into the app — `SideritaController` invokables → `siderita-ops`, view refresh on success, a truthful `op_error` on failure
- [x] Core verbs: new folder / new file, rename, copy, move, delete-to-Trash — wired end-to-end (verified: a headless self-test drove create → rename → trash through the bridge and the filesystem matched)
- [x] Keyboard verbs: F2 rename, Delete → Trash, Ctrl+C / X / V
- [x] Cut / copy / paste inside Siderita (an internal clipboard) + a shared new-folder / new-file / rename prompt
- [x] Freedesktop Trash support (send; restore is CP2)
- [x] Guarantee: a source is never removed after a partial copy or without revalidation — domain-enforced and tested, including the cross-device cancel path
- [ ] System-clipboard interop — the URI-list + desktop file-clipboard convention, so paste works to and from other managers
- [x] An async operation executor: paste (copy / move) runs on a worker thread with a progress surface (current entry, top-level count, bytes) and a **Cancel** button that trips the cancellation token — a cancelled cross-device move still leaves every source intact; a second paste is refused while one runs, and the filesystem-mutating shortcuts/menu items are disabled meanwhile. Destination collisions are detected up front and raise a **skip / replace / keep-both** dialog (choice applied to the batch): replace sends the existing entry to Trash first (recoverable, nothing hard-deleted), keep-both places a freed `(copia)` name via the new loss-free `copy_as`/`move_as` primitives. Per-entry failures and a skipped count are surfaced. (Per-collision granularity and live progress/cancel interaction — the latter awaiting a real Wayland session — are the remaining polish.)
- [x] Undo the last operation (move / rename / trash) — `Ctrl+Z` (or the empty-space menu, labelled for what it reverses) reverses the last rename, cut-paste move or send-to-Trash; trash undo uses the new `siderita_ops::restore_from_trash` primitive. Single level, batch-aware (a multi-trash restores every entry), and refuses to overwrite. Create and copy are deliberately not undoable and clear the pending undo on success
- [x] Multi-select batch operations — copy, cut and send-to-Trash act on the whole selection when the right-clicked/focused entry is part of a multi-selection (else the single entry); each entry is attempted independently, the view refreshes once so successes appear, and failures are reported together (`N de M operaciones fallaron`). A partial cut keeps only the entries it could not move on the clipboard, so a retry never re-moves a relocated one
- [x] Activate a file → open with its default application (xdg-open) — double-click or the entry menu's "Abrir" hands the path to the desktop's handler, detached and reaped, with a truthful `op_error` if the launcher can't start; the Open-with… chooser and default-app management are CP2

## Checkpoint 2 — Interoperable daily manager (S2)
**Goal:** a manager good enough for daily use, integrated through standards.

- [ ] XDG Trash restore, cross-filesystem moves, and removable-volume mount / unmount (the sidebar "removable files" the purpose promises) — the loss-free restore *primitive* (`siderita_ops::restore_from_trash`, reads the `.trashinfo`, refuses to overwrite) landed with CP1 undo; a Trash-browsing view to invoke it, plus mount/unmount, are still open here
- [ ] Drag-and-drop to move / copy within the view and to and from other applications
- [ ] Open-with… chooser, set-default-application, and safe `.desktop` handler wiring
- [ ] `org.freedesktop.FileManager1` D-Bus, so "Show in file manager" from other apps lands here
- [ ] Full accessibility (screen reader, focus order, contrast, animations) and daily-use resource budgets
- [x] Consume the shared CelestinaStyle tokens, glass and fallback icons — the theme/glass now live canonically in `../celestina-style` and are compiled into this module; Siderita's private copies were removed (verified: builds + offscreen run clean). Installed-release form is tracked in CelestinaStyle CP0.

## Checkpoint 3 — Comfortable daily manager (S3)
**Goal:** the comforts a manager is expected to have, each earned by a
demonstrated daily need and weighed against the resource budget — added one at a
time, never as a batch for parity.

- [ ] Properties / Get-Info — permissions, owner, MIME, timestamps, symlink target — with recursive folder size
- [ ] Details / columns view with sortable size / date / type columns, beyond today's single subtitle line
- [ ] Recursive filename search — a bounded, cancellable, non-indexed directory walk that is truthful about the scope it covered
- [ ] Thumbnails + a spacebar quick-look preview (images / video / PDF) — gated hardest of all, since it adds the freedesktop thumbnail spec and a cache to the closure; ships only on a proven need
- [ ] "Open terminal here" — launching the desktop's terminal, not an embedded one

## Non-goals

No cloud/network, global indexer, archive VFS, plugins, IDE, terminal or suite
daemon, and no features of other applications, before the declared local manager
is complete.

The boundary, not a loophole: CP3's bounded non-indexed filename search is not the
global indexer ruled out here, and "open terminal here" launches an external
terminal rather than embedding one. Everything else on this list stays out until
the plain local manager (CP0–CP2) is complete and a daily need is shown.
