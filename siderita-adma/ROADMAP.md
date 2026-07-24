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
context menus, sidebar places (XDG), bookmarks and tabs — and CP1 is now
functionally complete: the loss-free write verbs (new folder/file, rename,
copy, move, send-to-Trash) plus file activation (xdg-open), multi-select batch
operations, single-level undo backed by a freedesktop Trash-restore primitive,
an async paste executor (worker thread, progress surface, cancellation and a
skip/replace/keep-both conflict dialog) and system-clipboard interop are all
wired from the `siderita-ops` crate, each verb refusing to overwrite and never
removing a source before its destination is verified. Installation staging,
watcher, `file://`, a native role-based model, UI tests and real-Wayland
blur/frame numbers are still open, so Qt/QML stays a **provisional** first
iteration, and CP1's live interaction (progress/cancel, conflict dialog,
cross-manager clipboard) still awaits a real Wayland session to validate. The arc from here is deliberate: **operations** (CP1) make it a
manager, **interoperation** (CP2) make it a good desktop citizen, and **comfort**
(CP3) adds what a daily manager is expected to have — each feature earned by a
demonstrated need, never by parity.

**Key decisions.** Siderita keeps its own roadmap and release; the Rust cores live
in a separate workspace so each domain is testable without a toolkit; C++ is
limited to the CXX-Qt bridge plus one small hand-written shim for the system
clipboard (`QClipboard`/`QMimeData`, absent from cxx-qt-lib); internal glass
lives in QML (bounded capture +
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
- [x] Adjustable item size — a bottom-bar "Tamaño" button opens a glass submenu of independent, **persisted** size sliders (10 %–100 %, content icons to 150 %), split across three areas × icons/text: content, interface (breadcrumb / search / tabs / bottom bar) and sidebar. The content sliders scale the list rows / grid cells, glyphs, icons and labels; the grid columns stretch to fill the width
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
- [x] Fix truthful-state gap: a failed navigation via back/forward/up/home/activate must not leave the path pointing at an unreadable directory while the list still shows the previous one — **all navigation now commits on success**. Every verb (back / forward / up / home / activate / typed path) peeks its destination without mutating history (`NavigationHistory::peek_back` / `peek_forward`), scans it, and the history change is applied by a `PendingNav { Back, Forward, To }` only when the scan succeeds; on failure the path bar rolls back to where the history still is. Core peek is unit-tested
- [x] Local `file://` URI handling — the path bar and the argv/initial location accept a `file://` URI (percent-decoded, authority stripped, non-UTF-8-safe) via the shared `dbus::uri_to_path`, so a desktop "open with" or a pasted URI resolves to its local path; a bare name that merely starts with "file" is left alone. Unit-tested
- [x] Watcher wired to `WatchState` (invalidate + rescan wins) — a `notify`-backed (inotify) debouncer (full, event-kind-aware) watches the current folder non-recursively and coalesces bursts (200 ms); a change marshals to the Qt thread, `WatchState::observe_change` marks the snapshot stale, and a fresh **quiet** rescan wins (keeps the list/selection/status on screen — no loading flash). `Access` events (open/close/read) are ignored, so the scan's own `read_dir` (which notify reports as `IN_OPEN`) can't feed a scan→open→scan loop. Navigation moves the watch; a rescan of the same folder just `mark_rescanned`s it. A lost watch (`degrade`) flips a truthful "⚠ Vigilancia perdida · instantánea" status. Verified end-to-end against real create/remove events
- [x] Replace `QStringList` with a native role-based `QAbstractListModel`, dropping the per-delegate token/kind/subtitle invokables and the `viewRevision` workaround — done. Since cxx-qt 0.9.1 offers no `QAbstractListModel` virtual overrides from Rust, the model is a hand-written moc'd C++ class (`cpp/entrymodel.*`, `name`/`token`/`kind`/`subtitle`/`path`/`isDirectory` roles, `beginResetModel`) registered into the QML module; the controller pushes each projected view to it through a single `rowsReady` signal (parallel role columns), and the list/grid delegates read **roles** instead of calling `entryKind`/`entrySubtitle`/`entryIsDirectory` per row. The `viewRevision` counter is gone — the model's own reset signal drives the selection re-sync. (`entry_token` / `index_for_token` / `entry_names` remain only as the selection / type-ahead query API, not the model.) Verified end-to-end: the delegates use `required property` roles, so a clean Wayland load with a populated HOME proves every role is served (a missing one would error loudly)
- [x] Give the grid view keyboard navigation (only the list handled keys before) — the grid now mirrors the list: ←/→ move by cell, ↑/↓ by a full row (± the live column count), Home/End, PageUp/PageDown (rows×cols), Backspace = up a folder, Enter activates, Space selects, and type-ahead jumps to the next matching name — each keeping the focused cell in view and the selection in sync
- [x] Staged install with an allowlist (Basic + only the plugins actually used) — `scripts/stage-i1.sh` stages the binary + an **allowlist** of QML modules (QtQml, QtQuick, QtQuick.Controls + **Basic** + impl, Templates, Effects, Layouts, Window) and plugins (wayland/xcb/offscreen platforms + wayland client integrations, SVG image-format + icon-engine) plus the **transitive Qt `.so` closure** (fixed-point `ldd` over the binary and every copied plugin), with a launcher that points `QML_IMPORT_PATH` / `QT_PLUGIN_PATH` / `LD_LIBRARY_PATH` at the stage. Verified self-contained: in a stripped env it loads every `libQt6*` and the Basic-style plugin from the stage, **zero from `/usr/lib`**
- [x] Real-Wayland validation: keyboard, contrast, animations, themed icons; blur on/off frame p95 ≤ 16.7 ms, measured three times — validated on the maintainer's real Wayland session: the functional pass (keyboard, contrast, animations, themed icons) checks out and the resource-budget + blur-frame measurement runs (`measure-i1.sh`, ×3) were completed
- [x] Ratify Qt/QML for the suite, or reopen the frontend decision, from the data — **ratified**: the measured read-only slice met its budgets and the frontend holds, so Qt/QML is confirmed for the suite

### Provisional budget

| Metric | Limit | Current cut |
|---|---:|---:|
| Stripped isolated binary | 20 MiB | 4.23 MiB (was 1.62; +CP1/CP2 incl. zbus); staging pending |
| First install closure (Qt) | 250 MiB | 50 MiB staged (allowlist; `scripts/stage-i1.sh`) |
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

The write-side domain lives in the `siderita-ops` crate — create, rename, copy,
move, send-to-Trash, restore-from-Trash and exact-name `copy_as`/`move_as`, all
pure and toolkit-free, tested. Every verb refuses to overwrite and never
removes a source before its destination is verified. Every item below is
implemented **and** the live interaction (progress/cancel, the conflict dialog
and cross-manager clipboard) is validated on a real Wayland session — the
checkpoint is complete.

- [x] Wire the write-side domain into the app — `SideritaController` invokables → `siderita-ops`, view refresh on success, a truthful `op_error` on failure
- [x] Core verbs: new folder / new file, rename, copy, move, delete-to-Trash — wired end-to-end (verified: a headless self-test drove create → rename → trash through the bridge and the filesystem matched)
- [x] Keyboard verbs: F2 rename, Delete → Trash, Ctrl+C / X / V
- [x] Cut / copy / paste inside Siderita (an internal clipboard) + a shared new-folder / new-file / rename prompt
- [x] Freedesktop Trash support (send; restore is CP2)
- [x] Guarantee: a source is never removed after a partial copy or without revalidation — domain-enforced and tested, including the cross-device cancel path
- [x] System-clipboard interop — copy / cut now also publish to the system clipboard as `text/uri-list` + `x-special/gnome-copied-files` (the convention other managers honour), and paste reads file URIs from the system clipboard (the shared source of truth, with the internal one as fallback), so paste works to and from other managers; a consumed cut clears it. Implemented via a small hand-written `QClipboard`/`QMimeData` shim (`cpp/clipboard.cpp`) since cxx-qt-lib exposes neither. Cross-manager copy/cut/paste is validated on a real Wayland session (user-tested).
- [x] An async operation executor: paste (copy / move) runs on a worker thread with a progress surface (current entry, top-level count, bytes) and a **Cancel** button that trips the cancellation token — a cancelled cross-device move still leaves every source intact; a second paste is refused while one runs, and the filesystem-mutating shortcuts/menu items are disabled meanwhile. Destination collisions are detected up front and raise a **skip / replace / keep-both** dialog (choice applied to the batch): replace sends the existing entry to Trash first (recoverable, nothing hard-deleted), keep-both places a freed `(copia)` name via the new loss-free `copy_as`/`move_as` primitives. Per-entry failures and a skipped count are surfaced. The live progress + cancel interaction is validated on a real Wayland session (user-tested); per-collision granularity (versus the batch-wide choice) is the one remaining refinement.
- [x] Undo the last operation (move / rename / trash) — `Ctrl+Z` (or the empty-space menu, labelled for what it reverses) reverses the last rename, cut-paste move or send-to-Trash; trash undo uses the new `siderita_ops::restore_from_trash` primitive. Single level, batch-aware (a multi-trash restores every entry), and refuses to overwrite. Create and copy are deliberately not undoable and clear the pending undo on success
- [x] Multi-select batch operations — copy, cut and send-to-Trash act on the whole selection when the right-clicked/focused entry is part of a multi-selection (else the single entry); each entry is attempted independently, the view refreshes once so successes appear, and failures are reported together (`N de M operaciones fallaron`). A partial cut keeps only the entries it could not move on the clipboard, so a retry never re-moves a relocated one
- [x] Activate a file → open with its default application (xdg-open) — double-click or the entry menu's "Abrir" hands the path to the desktop's handler, detached and reaped, with a truthful `op_error` if the launcher can't start; the Open-with… chooser and default-app management are CP2

## Checkpoint 2 — Interoperable daily manager (S2)
**Goal:** a manager good enough for daily use, integrated through standards.

- [x] XDG Trash restore, cross-filesystem moves, and removable-volume mount / unmount (the sidebar "removable files" the purpose promises) — **Trash restore**: `siderita_ops::list_home_trash` + the loss-free `restore_from_trash` primitive back a sidebar "Papelera" view (name · origin · date, newest first) with per-entry Restore, Restaurar todo, and a confirm-first **Vaciar** that permanently empties it (the new deliberate-loss `siderita_ops::purge_from_trash` primitive, with 3 unit tests). **Cross-filesystem moves** work at the domain level (`relocate_by_copy`: copy → verify → remove-source, used by move/paste/drop). **Removable volumes**: a "Dispositivos" sidebar section (`volumes.rs` over UDisks2 on the system bus, `zbus`) lists the removable filesystems — verified against real hardware, it picked the two external USB drives and excluded every internal NVMe partition, matching `lsblk` — with click-to-open (mounting first if needed) and an eject/unmount control. **Mount/unmount run on a worker thread** (marshalled back to the Qt thread), so a polkit authorization prompt never freezes the UI; the list reloads into the active tab on tab switch, **and hotplug auto-refresh is wired**: `volumes::watch_changes` subscribes to the UDisks2 ObjectManager's InterfacesAdded / InterfacesRemoved signals (a plug exposes several interfaces at once, so each burst is coalesced over a 300 ms quiet window) and re-enumerates on the Qt thread, so plugging or unplugging a drive updates the list on its own
- [x] Drag-and-drop to move / copy within the view and to and from other applications — **drop-in + internal move done**: files dragged from another application drop into the current folder or onto a folder row/cell (list + grid; external default copy, Shift = move); and now **any entry is draggable within the view** — drag a file or folder onto another folder to move it there (Ctrl = copy), while a folder dragged to the sidebar still bookmarks (keyed drags: every entry carries `siderita-entry`, only folders carry `siderita-bookmark`). All routed through the same collision-detection + async worker + conflict dialog (`controller.drop_uris`). A fixed z-order bug that let the view-level drop target shadow the per-folder ones is gone. **Drag-out** now implemented too: the drag is `Drag.Automatic` and carries a `text/uri-list` `file://` URI, so an entry can be dragged into other applications; internal targets still dispatch on the `siderita-entry` key first (so an internal move stays a move, not a URI-copy). Loads clean on Wayland; the live drag gestures (internal move, bookmark, and now cross-app drag-out) are user-tested. The drag image is now the entry's icon grabbed into `Drag.imageSource` so the compositor renders it at the cursor — replacing a manually-positioned QML ghost that a native (`Drag.Automatic`) drag left stranded at the top-left.
- [x] Open-with… chooser, set-default-application, and safe `.desktop` handler wiring — the entry menu's "Abrir con…" classifies the file via `xdg-mime` (the desktop's own database, not a reimplemented shared-mime-info), lists the applications whose `.desktop` `MimeType=` declares it (parsed in `apps.rs`, user dirs shadowing system, `NoDisplay`/`Hidden` filtered), badges the current default, and launches the chosen one via `gtk-launch` — with an optional "Predeterminar y abrir" that sets the default via `xdg-mime default`. Detection/default/candidate-list verified against the real database; the launch and set-default *actions* are validated on a real Wayland session (user-tested).
- [x] `org.freedesktop.FileManager1` D-Bus, so "Show in file manager" from other apps lands here — a background thread (new `zbus` dep) serves `ShowFolders` / `ShowItems` / `ShowItemProperties` on the session bus and marshals each onto the Qt thread as a signal the window turns into a foreground tab (raising the window). Best-effort and polite: it requests the name without replacing an existing owner, so it only receives calls when it is the session's manager. Verified on the live session bus (introspection + a `ShowFolders` call re-checked by the maintainer), and when the name is already owned it queues without error. The one thing still gated on the environment is seeing an actual tab open — that needs Siderita to be the session's FileManager1 owner (another manager holds the name by default). `uri_to_path` is unit-tested (4 tests)
- [x] Full accessibility (screen reader, focus order, contrast, animations) and daily-use resource budgets — the new CP2 surfaces are labelled (`Accessible.Button`/`Accessible.Dialog`/`ListItem` with names + selected state) and **keyboard-operable**: the Abrir-con and Papelera list dialogs take ↑/↓ to move the selection/focus (scrolled into view), Enter to open/restore, Escape to close; every dialog grabs focus while shown. The screen-reader pass, the Tab focus-order audit across the main view / sidebar / tabs, the contrast/reduced-motion review and the daily-use resource budgets are validated on a real AT + Wayland session (user-tested)
- [x] Consume the shared CelestinaStyle tokens, glass and fallback icons — the theme/glass now live canonically in `../celestina-style` and are compiled into this module; Siderita's private copies were removed (verified: builds + offscreen run clean). Installed-release form is tracked in CelestinaStyle CP0.

## Checkpoint 3 — Comfortable daily manager (S3)
**Goal:** the comforts a manager is expected to have, each earned by a
demonstrated daily need and weighed against the resource budget — added one at a
time, never as a batch for parity.

- [x] Properties / Get-Info — permissions, owner, MIME, timestamps, symlink target — with recursive folder size — the entry menu's "Propiedades" opens a panel with name, path, kind, MIME (`xdg-mime`), size, `rwxr-xr-x` permissions, `user · group` owner (resolved from `/etc/passwd`+`/etc/group`), local-time modified/accessed (`localtime_r`), and the symlink target when it is one. A **folder's recursive size** is walked on a worker thread (cancellable, symlink-safe, cancelled when the panel closes or moves on) so a deep tree never blocks the UI. Domain `properties.rs` with unit-tested formatters; gather + walk verified against a real file, symlink and directory
- [ ] Details / columns view with sortable size / date / type columns, beyond today's single subtitle line
- [x] Recursive filename search — a bounded, cancellable, non-indexed directory walk that is truthful about the scope it covered — typing filters the current folder live; a **Subcarpetas** button (or ⏎) walks it (case-insensitive name match) on a worker thread, capped at 500 hits and never following symlinks (no loops/escape). The hits ride the **same entryModel and delegates** as the folder, so the list/grid render and act on them identically — single-click selects, double-click opens (a folder navigates in, a file opens), keyboard, selection — the search results *are* the content view (a slim glass bar carries the query, summary and Stop/Close; Escape/Back close it). The subtitle is the hit's containing folder. Its summary states exactly what happened — "N carpetas exploradas", "detenida en el límite", or "búsqueda detenida" when cancelled. Domain `search.rs` with 3 unit tests (recursive match, cap→truncated, empty query)
- [ ] Thumbnails + a spacebar quick-look preview (images / video / PDF) — gated hardest of all, since it adds the freedesktop thumbnail spec and a cache to the closure; ships only on a proven need
- [x] "Open terminal here" — launching the desktop's terminal, not an embedded one — the folder menu's "Abrir terminal aquí" spawns an external terminal with its working directory set to the current folder, honouring `$TERMINAL` then a list of common emulators (foot/alacritty/kitty/wezterm/gnome-terminal/konsole/xfce4-terminal/xterm), the first installed one winning; detached and reaped, with a truthful `op_error` if none is found. No embedded terminal — the CP3 boundary holds

## Non-goals

No cloud/network, global indexer, archive VFS, plugins, IDE, terminal or suite
daemon, and no features of other applications, before the declared local manager
is complete.

The boundary, not a loophole: CP3's bounded non-indexed filename search is not the
global indexer ruled out here, and "open terminal here" launches an external
terminal rather than embedding one. Everything else on this list stays out until
the plain local manager (CP0–CP2) is complete and a daily need is shown.
