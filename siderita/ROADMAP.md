# Siderita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the file
> manager only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is
> not "verified": real-Wayland acceptance is tracked as its own goal — and it is
> the one CP4 leaves open, because a drag gesture and a blur have no headless
> proof.

## Version 1.0 — Iteration 1, concluded

*2026-07-25.* Iteration 1 is the whole arc, CP0 → CP5: a truthful read-only
slice grown into a loss-free file manager (CP1), a good desktop citizen (CP2), a
comfortable daily driver (CP3), one that holds the user's arrangement (CP4), and
finally the desktop's own file chooser (CP5). That arc is done and tagged
**v1.0** — the Qt/QML host that was a *provisional* first iteration is now the
shipped one, its name no longer carrying the iteration marker (`qml/`, crate
`siderita`, not `siderita-i1`).

What stays open is named, not hidden — three items that are input- or
pixel-shaped, so they have no headless proof and are carried past 1.0 rather than
claimed as verified:

- **CP4 real-Wayland validation** — the drag gestures (spring-open, edge scroll,
  sidebar reorder) and the live-capture menu-blur p95.
- **CP5 window parenting** — `parent_window` needs `xdg-foreign`; the picker
  floats free until then.
- **CP5 daily-use validation** — portal routing stays opt-in in `portals.conf`
  until it has been lived with, because a bad file chooser is every
  application's upload button.

Everything else in CP0–CP5 below is implemented, and the parts with a headless or
unit-test proof are marked verified where they stand.

**Visual migration (2026-07-28).** The first One UI 8.5 desktop composition is
live: the sidebar, tab strip and file region are opaque tonal work surfaces;
the path/search bar and consolidated footer are the denser floating glass layer;
and page hierarchy now starts with a large location heading. List/grid switching,
tabs, navigation, sorting, filtering and file operations retain their existing
controllers and signals. Successful route changes reveal the new heading, path
and entries with the shared motion curve; watcher refreshes stay still. The base
composition was verified in a real Wayland session in both list and grid modes.
The later host-controlled `reducedMotion` path now makes `RouteReveal` and the
touched picker transitions instant, but that correction has automated/static
evidence only: remaining legacy motion and the full enabled/disabled interaction
pass are still open.

**A floating surface owns its pointer (2026-07-31).** Everything painted over
the scrolling listing was passing input through to the rows it hides: hover lit
a row nobody could see, the three mouse buttons acted on it (a right click
opened *that file's* menu, middle click opened it in a tab) and a sweep started
its file drag. Only `GlassPill`, `FloatingButton` and the modal layer had ever
declared an input floor, and all three were incomplete in the same way — see the
drag note below. That recipe is now one shared type,
`CelestinaStyle`'s `CelestinaInputShield`, consumed here through the usual
symlink. Adopted by the pills (`GlassPill`, `FloatingButton`, `InfoPill`), the
buttons of the search / Recientes / Papelera strips — which floated bare over
the listing, inside no box at all — the details column header, both `TopBar`
pills, the tab chips and the new-tab pill, the footer's error banners and
operation-progress panel, and the size popup (a non-modal `Popup` keeps hover
and clicks but not the drag). Verified by simulated input rather than by
reading: `scripts/qml-tests.sh` drives `qmltestrunner` over the real components
(31 cases), and the surface cases fail against the unshielded tree. Still
unverified in a compositor: touch, tablet and any pointer path a synthetic mouse
does not reproduce.

**The drag was the half that survived (2026-07-31).** Swallowing clicks is not
blocking a surface. Reported from real use: with the properties dialog open, a
click-drag on an empty part of the card moved the file underneath, and the same
happened from the floating pills. The reason is that a `DragHandler` below takes
a passive grab on the press and asks for the exclusive one a few pixels into the
sweep — before a shield with an ordinary drag threshold asks for anything — and
an item that merely accepts clicks (a dialog card's `MouseArea`, the path pill's)
does not refuse that takeover. The shield now claims the drag on the press
itself, so the handler underneath never gets it, while a text field inside the
surface still selects and a button still clicks. Two shapes reproduce it and are
now covered: pressing on empty card space, and any sweep that leaves the box on
its way to the listing.

Two defects fell out of that work. The details view's **column headers were
never clickable** — the header row had no height of its own, so its sort hit
areas were zero-tall and clicking `Nombre` / `Tamaño` / `Fecha` / `Tipo` did
nothing; the labels looked right because they are centred by anchors. Sorting by
clicking a column now works and is covered by a test. And a `blocking`
`HoverHandler` next to a `hoverEnabled` `MouseArea` is **not disarmed by
`enabled: false` on the parent item**: hover is still delivered to a disabled
item, so each condition rides the property that governs it.

## Overview

**Purpose.** The suite's independent file manager — navigate, organize and
retrieve local and removable files, integrated through freedesktop standards. No
editor, viewer, player, panel or dotfiles manager inside it; those are separate
projects reached through desktop standards.

**Current state.** The earlier C++/Qt prototype was removed; the Rust host of
`qml` is the only implementation. It consumes the `celestina-rs` domain crates
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
removing a source before its destination is verified. The items that once
kept this provisional — installation staging, the hotplug/FS watcher, `file://`,
a native role-based model, real-Wayland resource/frame numbers — are done, so
Qt/QML is **ratified** as the shipped first iteration (v1.0); only the input- and
pixel-shaped validation named at the top of this file is still to be lived with
on the real session. The arc from here is deliberate: **operations** (CP1) make it a
manager, **interoperation** (CP2) make it a good desktop citizen, **comfort**
(CP3) adds what a daily manager is expected to have, and **arrangement** (CP4)
hands the sidebar, the order, the per-folder view and the session back to the
user — each feature earned by a demonstrated need, never by parity.

**Key decisions.** Siderita keeps its own roadmap and release; the Rust cores live
in a separate workspace so each domain is testable without a toolkit; C++ is
limited to the CXX-Qt bridge plus one small hand-written shim for the system
clipboard (`QClipboard`/`QMimeData`, absent from cxx-qt-lib); internal glass
lives in QML (bounded capture +
`MultiEffect`, translucent fallback); semantic/freedesktop icon names resolve
through the shared closed Lucide catalogue; Qt dependencies stay under an allowlist; the visible name is never
identity (homonyms, rename and non-UTF-8 names are preserved); a source is never
deleted before its destination is verified; integration is via XDG/freedesktop;
shared style is symlink-compiled from the canonical `celestina-style` source,
never copied or forked locally.

## Checkpoint 0 — Truthful, measured read-only slice
**Goal:** a staged install opens HOME, a path, or a local URI in a modern
read-only view; a context menu demonstrates real in-scene glass; and the
resource report ratifies or rejects Qt/QML with data.

### Implemented
- [x] `celestina-core` / `siderita-core` / `siderita-qt` neutral up to the Qt edge (PathBuf/OsString, EntryId, generations, opaque tokens)
- [x] Bounded scan executor with cancellation and deterministic shutdown
- [x] Qt Quick/QML UI + minimal CXX-Qt adapter (provisional)
- [x] Content layer separated from overlay + a shareable `GlassSurface` (bounded capture, no work at all when the surface is closed). The capture is now **live** for menus as well as modals: a one-shot snapshot froze the instant the surface opened, so anything scrolling, hovering or loading behind it turned the glass into a blurred screenshot. The sampled region follows the surface's size and position too, so a menu that grows as its items decide to be visible no longer stretches a stale region
- [x] HOME / path navigation (back / forward / up / home / refresh) incl. mouse side buttons
- [x] Filter with 120 ms debounce; sort by name / size / date / kind, both directions, folders first; stable selection across re-sort; hidden toggle
- [x] Segmented breadcrumb + list/grid view toggle
- [x] Adjustable item size — a bottom-bar "Tamaño" button opens a glass submenu of independent, **persisted** literal scale sliders (`1.0 = 100 %`): content icons 75 %–150 %, fixed-anatomy chrome/sidebar icons 75 %–125 %, and text 20 %–200 %. A versioned migration corrects the former doubled icon factors once without changing text. The content sliders scale the list rows / grid cells, glyphs, icons and labels; the grid columns stretch to fill the width
- [x] Multi-selection — plain / Ctrl / Shift click, a drag-marquee zone, right-click-selects and select-all; the status line shows the count; token-keyed so it survives sort/filter and clears on navigation
- [x] Per-item and empty-space (folder) context menus with the glass background
- [x] Sidebar places — Inicio plus the standard XDG user folders (Escritorio, Documentos, Descargas, Música, Imágenes, Vídeos), resolved from `user-dirs.dirs` and shown only when they exist
- [x] Sidebar bookmarks — add from a folder's context menu or by dragging it onto the sidebar, rename, reorder (drag a row, or Subir / Bajar from its menu for the keyboard), remove, navigate; persisted across restarts to `~/.config/siderita/bookmarks.tsv`
- [x] Header moved into the content box as one floating glass path/search bar; controls consolidated into a glass footer; sidebar bottom info box
- [x] Tabs — open a folder in a new tab (middle-click or its context menu), shown in a scrollable tonal strip below the path/search bar; each tab is an independent navigation context (its own history, scan worker and selection), closable via × / middle-click, with `Ctrl+T` / `Ctrl+W` / `Ctrl+Tab`
- [x] Host-side Rust for `entry_path`, the bookmark store and the XDG places resolver, with unit tests (bookmark round-trip + sanitization, `user-dirs.dirs` parsing)
- [x] Truthful loading / empty / error / degraded-watch states
- [x] Vendored Lucide icons behind compatibility aliases; no desktop icon-theme dependency
- [x] Dependency inventory + size / memory / CPU / threads baselines (offscreen)

### To finish CP0
- [x] Fix truthful-state gap: a failed navigation via back/forward/up/home/activate must not leave the path pointing at an unreadable directory while the list still shows the previous one — **all navigation now commits on success**. Every verb (back / forward / up / home / activate / typed path) peeks its destination without mutating history (`NavigationHistory::peek_back` / `peek_forward`), scans it, and the history change is applied by a `PendingNav { Back, Forward, To }` only when the scan succeeds; on failure the path bar rolls back to where the history still is. Core peek is unit-tested
- [x] Local `file://` URI handling — the path bar and the argv/initial location accept a `file://` URI (percent-decoded, authority stripped, non-UTF-8-safe) via the shared `dbus::uri_to_path`, so a desktop "open with" or a pasted URI resolves to its local path; a bare name that merely starts with "file" is left alone. Unit-tested
- [x] Watcher wired to `WatchState` (invalidate + rescan wins) — a `notify`-backed (inotify) debouncer (full, event-kind-aware) watches the current folder non-recursively and coalesces bursts (200 ms); a change marshals to the Qt thread, `WatchState::observe_change` marks the snapshot stale, and a fresh **quiet** rescan wins (keeps the list/selection/status on screen — no loading flash). `Access` events (open/close/read) are ignored, so the scan's own `read_dir` (which notify reports as `IN_OPEN`) can't feed a scan→open→scan loop. Navigation moves the watch; a rescan of the same folder just `mark_rescanned`s it. A lost watch (`degrade`) flips a truthful "⚠ Vigilancia perdida · instantánea" status. Verified end-to-end against real create/remove events
- [x] Replace `QStringList` with a native role-based `QAbstractListModel`, dropping the per-delegate token/kind/subtitle invokables and the `viewRevision` workaround — done. Since cxx-qt 0.9.1 offers no `QAbstractListModel` virtual overrides from Rust, the model is a hand-written moc'd C++ class (`cpp/entrymodel.*`, `name`/`token`/`kind`/`subtitle`/`path`/`isDirectory` roles, `beginResetModel`) registered into the QML module; the controller pushes each projected view to it through a single `rowsReady` signal (parallel role columns), and the list/grid delegates read **roles** instead of calling `entryKind`/`entrySubtitle`/`entryIsDirectory` per row. The `viewRevision` counter is gone — the model's own reset signal drives the selection re-sync. (`entry_token` / `index_for_token` / `entry_names` remain only as the selection / type-ahead query API, not the model.) Verified end-to-end: the delegates use `required property` roles, so a clean Wayland load with a populated HOME proves every role is served (a missing one would error loudly)
- [x] Give the grid view keyboard navigation (only the list handled keys before) — the grid now mirrors the list: ←/→ move by cell, ↑/↓ by a full row (± the live column count), Home/End, PageUp/PageDown (rows×cols), Backspace = up a folder, Enter activates, Space selects, and type-ahead jumps to the next matching name — each keeping the focused cell in view and the selection in sync
- [x] Staged install with an allowlist (Basic + only the plugins actually used) — `scripts/stage.sh` stages the binary + an **allowlist** of QML modules (QtQml, QtQuick, QtQuick.Controls + **Basic** + impl, Templates, Effects, Layouts, Window) and plugins (wayland/xcb/offscreen platforms + wayland client integrations, SVG image-format + icon-engine) plus the **transitive Qt `.so` closure** (fixed-point `ldd` over the binary and every copied plugin), with a launcher that points `QML_IMPORT_PATH` / `QT_PLUGIN_PATH` / `LD_LIBRARY_PATH` at the stage. Verified self-contained: in a stripped env it loads every `libQt6*` and the Basic-style plugin from the stage, **zero from `/usr/lib`**
- [x] Real-Wayland validation: keyboard, contrast, animations, icons; blur on/off frame p95 ≤ 16.7 ms, measured three times — validated on the maintainer's real Wayland session: the functional pass checks out and the resource-budget + blur-frame measurement runs (`measure.sh`, ×3) were completed
- [x] Ratify Qt/QML for the suite, or reopen the frontend decision, from the data — **ratified**: the measured read-only slice met its budgets and the frontend holds, so Qt/QML is confirmed for the suite

> Packaging note (2026-07-27): the `install.sh` / `stage.sh` / `measure.sh`
> scripts referenced above were consolidated into a single `scripts/run.sh`
> (build in release + install to `~/.local`). The self-contained staged install
> and the resource-measurement tooling were retired for the single-author,
> system-Qt setup; the measurements they produced stand as the record here.

### Provisional budget

| Metric | Limit | Current cut |
|---|---:|---:|
| Stripped isolated binary | 20 MiB | 5.60 MiB (4.23 before CP4); staging pending |
| First install closure (Qt) | 250 MiB | 50 MiB staged (allowlist; `scripts/stage.sh`) |
| HOME mean PSS, 60 s | 120 MiB | 86.2 MiB (82.1 before CP4), offscreen |
| HOME one-core CPU, 60 s | 1 % | ~0.1 %, offscreen, idle |
| 10k-entry fixture mean PSS | 250 MiB | 110.9 MiB (105.1 before CP4), offscreen |
| 10k-entry re-sort | — | ~16 ms per flip (full re-project + republish) |
| 10k-entry filter keystroke | — | ~9 ms |
| Menu blur frame p95 | 16.7 ms | **re-measure**: menus capture live since CP4 |

Re-measured 2026-07-24 on the offscreen backend, one run per scenario, each
figure taken against the same build it is compared with (the "before CP4" column
is HEAD built from a worktree, so the delta is CP4's and not four months of
drift). The old 40.52 / 46.47 MiB figures dated from 2026-07-20, before CP1–CP3
landed; the budget is still met with room, but the headroom is now half of what
those numbers implied. Not a substitute for three runs in a real Wayland session;
GPU memory and the first-install Qt closure are not yet counted. Limits may tighten with evidence;
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
- [x] An async operation executor: paste (copy / move) runs on a worker thread with a progress surface (current entry, top-level count, bytes) and a **Cancel** button that trips the cancellation token — a cancelled cross-device move still leaves every source intact; a second paste is refused while one runs, and the filesystem-mutating shortcuts/menu items are disabled meanwhile. Destination collisions are detected up front and raise a **skip / replace / keep-both** dialog (choice applied to the batch): replace sends the existing entry to Trash first (recoverable, nothing hard-deleted), keep-both places a freed `(copia)` name via the new loss-free `copy_as`/`move_as` primitives. Per-entry failures and a skipped count are surfaced. The live progress + cancel interaction is validated on a real Wayland session (user-tested). **Per-collision granularity is in** (CP4): each collision is now asked about on its own — the queue is decided before the worker starts, and one strategy per source rides into it — so a batch can skip one name and replace the next, with an explicit "apply to all" for the old batch-wide behaviour.
- [x] Undo the last operation (move / rename / trash) — `Ctrl+Z` (or the empty-space menu, labelled for what it reverses) reverses the last rename, cut-paste move or send-to-Trash; trash undo uses the new `siderita_ops::restore_from_trash` primitive. Single level, batch-aware (a multi-trash restores every entry), and refuses to overwrite. Create and copy are deliberately not undoable and clear the pending undo on success
- [x] Multi-select batch operations — copy, cut and send-to-Trash act on the whole selection when the right-clicked/focused entry is part of a multi-selection (else the single entry); each entry is attempted independently, the view refreshes once so successes appear, and failures are reported together (`N de M operaciones fallaron`). A partial cut keeps only the entries it could not move on the clipboard, so a retry never re-moves a relocated one
- [x] Activate a file → open with its default application (xdg-open) — double-click or the entry menu's "Abrir" hands the path to the desktop's handler, detached and reaped, with a truthful `op_error` if the launcher can't start; the Open-with… chooser and default-app management are CP2

## Checkpoint 2 — Interoperable daily manager (S2)
**Goal:** a manager good enough for daily use, integrated through standards.

- [x] XDG Trash restore, cross-filesystem moves, and removable-volume mount / unmount (the sidebar "removable files" the purpose promises) — **Trash restore**: `siderita_ops::list_home_trash` + the loss-free `restore_from_trash` primitive back a "Papelera" **content-view location** — the trashed entries ride the same entry model as a folder, so list / grid / details and thumbnails render them identically (name · origin · date), with per-entry Restore from the entry menu, Restaurar todo, a confirm-first **Vaciar** that permanently empties it, and Back / the mouse's back button to leave it (the new deliberate-loss `siderita_ops::purge_from_trash` primitive, with 3 unit tests). **Cross-filesystem moves** work at the domain level (`relocate_by_copy`: copy → verify → remove-source, used by move/paste/drop). **Removable volumes**: a "Dispositivos" sidebar section (`volumes.rs` over UDisks2 on the system bus, `zbus`) lists the removable filesystems — verified against real hardware, it picked the two external USB drives and excluded every internal NVMe partition, matching `lsblk` — with click-to-open (mounting first if needed) and an eject/unmount control. **Mount/unmount run on a worker thread** (marshalled back to the Qt thread), so a polkit authorization prompt never freezes the UI; the list reloads into the active tab on tab switch, **and hotplug auto-refresh is wired**: `volumes::watch_changes` subscribes to the UDisks2 ObjectManager's InterfacesAdded / InterfacesRemoved signals (a plug exposes several interfaces at once, so each burst is coalesced over a 300 ms quiet window) and re-enumerates on the Qt thread, so plugging or unplugging a drive updates the list on its own
- [x] Drag-and-drop to move / copy within the view and to and from other applications — **drop-in + internal move done**: files dragged from another application drop into the current folder or onto a folder row/cell (list + grid; external default copy, Shift = move); and now **any entry is draggable within the view** — drag a file or folder onto another folder to move it there (Ctrl = copy), while a folder dragged to the sidebar still bookmarks (keyed drags: every entry carries `siderita-entry`, only folders carry `siderita-bookmark`). All routed through the same collision-detection + async worker + conflict dialog (`controller.drop_uris`). A fixed z-order bug that let the view-level drop target shadow the per-folder ones is gone. **Drag-out** now implemented too: the drag is `Drag.Automatic` and carries a `text/uri-list` `file://` URI, so an entry can be dragged into other applications; internal targets still dispatch on the `siderita-entry` key first (so an internal move stays a move, not a URI-copy). Loads clean on Wayland; the live drag gestures (internal move, bookmark, and now cross-app drag-out) are user-tested. The drag image is now the entry's icon grabbed into `Drag.imageSource` so the compositor renders it at the cursor — replacing a manually-positioned QML ghost that a native (`Drag.Automatic`) drag left stranded at the top-left.
- [x] Open-with… chooser, set-default-application, and safe `.desktop` handler wiring — the entry menu's "Abrir con…" classifies the file via `xdg-mime` (the desktop's own database, not a reimplemented shared-mime-info), lists the applications whose `.desktop` `MimeType=` declares it (parsed in `apps.rs`, user dirs shadowing system, `NoDisplay`/`Hidden` filtered), badges the current default, and launches the chosen one via `gtk-launch` — with an optional "Predeterminar y abrir" that sets the default via `xdg-mime default`. Detection/default/candidate-list verified against the real database; the launch and set-default *actions* are validated on a real Wayland session (user-tested).
- [x] `org.freedesktop.FileManager1` D-Bus, so "Show in file manager" from other apps lands here — a background thread (new `zbus` dep) serves `ShowFolders` / `ShowItems` / `ShowItemProperties` on the session bus and marshals each onto the Qt thread as a signal the window turns into a foreground tab (raising the window). Best-effort and polite: it requests the name without replacing an existing owner, so it only receives calls when it is the session's manager. Verified on the live session bus (introspection + a `ShowFolders` call re-checked by the maintainer), and when the name is already owned it queues without error. The one thing still gated on the environment is seeing an actual tab open — that needs Siderita to be the session's FileManager1 owner (another manager holds the name by default). `uri_to_path` is unit-tested (4 tests)
- [x] Magnetita phone integration — the MÓVIL section keeps paired offline phones visible with a red status dot and dimmed label, turns the dot green while connected, and only navigates once storage is mounted. At the physical phone mount root —never in Recientes, Papelera or search results that happen to retain that path— the header becomes a compact, left-aligned device header with a media action; its modal consumes the additive `Devices1` media snapshot to show artwork, title, artist, timed progress and supported transport controls, while “Sonar” remains a separate device action. Verified on real Wayland against the Galaxy S25 Ultra with live YouTube metadata; `Alt+M` opens it and Escape closes it.
- [ ] Complete accessibility (screen reader, focus order, contrast, animations) and daily-use resource budgets — the CP2 surfaces are labelled (`Accessible.Button`/`Accessible.Dialog`/`ListItem` with names + selected state) and **keyboard-operable**: the Abrir-con and Papelera list dialogs take ↑/↓ to move the selection/focus (scrolled into view), Enter to open/restore, Escape to close; dialogs grab focus while shown. Screen-reader, Tab-order and resource-budget passes were exercised on real AT/Wayland for that slice. The newly implemented shared `reducedMotion` input has not yet had a complete real-session enabled/disabled pass, and several legacy animations still need conversion, so the broader claim remains open
- [x] Consume the shared CelestinaStyle tokens, glass and Lucide catalogue — theme/glass/icons now live canonically in `../celestina-style` and are compiled into this module; Siderita's private copies were removed (verified: builds + offscreen run clean). Installed-release form is tracked in CelestinaStyle CP0.

## Checkpoint 3 — Comfortable daily manager (S3)
**Goal:** the comforts a manager is expected to have, each earned by a
demonstrated daily need and weighed against the resource budget — added one at a
time, never as a batch for parity.

- [x] Properties / Get-Info — permissions, owner, MIME, timestamps, symlink target — with recursive folder size — the entry menu's "Propiedades" opens a panel with name, path, kind, MIME (`xdg-mime`), size, `rwxr-xr-x` permissions, `user · group` owner (resolved from `/etc/passwd`+`/etc/group`), local-time modified/accessed (`localtime_r`), and the symlink target when it is one. A **folder's recursive size** is walked on a worker thread (cancellable, symlink-safe, cancelled when the panel closes or moves on) so a deep tree never blocks the UI. Domain `properties.rs` with unit-tested formatters; gather + walk verified against a real file, symlink and directory
- [x] Details / columns view with sortable size / date / type columns, beyond today's single subtitle line — a third view mode (a segmented Lista / Cuadrícula / **Detalles** switch, persisted) renders each entry as aligned columns — **Nombre** (fills) · **Tamaño** · **Fecha** · **Tipo** — reusing the list rows' own selection / activate / drag / context-menu behaviour (only the row body swaps). A sticky glass header lines up with the columns; clicking a title sorts by that field and a second click flips the direction (a Lucide sort indicator marks the active one), driving the existing `sort_field` / `sort_direction`. Size (a dash for folders) and modified date ride two new `sizeText` / `dateText` model roles. Search results honour the chosen view like any other location (CP4) — only the details *columns* stay out of a search, since a hit carries no size or date
- [x] Recursive filename search — a bounded, cancellable, non-indexed directory walk that is truthful about the scope it covered — typing filters the current folder live; a **Subcarpetas** button (or ⏎) walks it (case-insensitive name match) on a worker thread, capped at 500 hits and never following symlinks (no loops/escape). The hits ride the **same entryModel and delegates** as the folder (and, since CP4, the same *view mode*), so the list/grid render and act on them identically — single-click selects, double-click opens (a folder navigates in, a file opens), keyboard, selection — the search results *are* the content view (a slim glass bar carries the query, summary and Stop/Close; Escape/Back close it). The subtitle is the hit's containing folder. Its summary states exactly what happened — "N carpetas exploradas", "detenida en el límite", or "búsqueda detenida" when cancelled. Domain `search.rs` with 3 unit tests (recursive match, cap→truncated, empty query)
- [x] Thumbnails + a spacebar quick-look preview — **image thumbnails + an image/text quick-look done**: an async `QQuickAsyncImageProvider` ("thumb", hand-written C++ like the entrymodel — registered onto the engine before the QML loads) backs `image://thumb/<path>` in the list / details / grid / search glyph tiles for raster image files, revealing the picture once decoded and keeping the generic glyph until then. It follows the freedesktop shared cache — 256 px "large" PNGs at `~/.cache/thumbnails/large/<md5(file-uri)>.png` — so it reuses (and contributes to) the cache other managers populate; validity keys off the filesystem mtime (a thumbnail is always newer than its source), which sidesteps Qt mangling the embedded `Thumb::MTime` key on write and also accepts other managers' thumbnails. Decoding is off the UI thread (QThreadPool, EXIF-aware, scaled-read), atomically cached. Verified: browsing an image folder generates and then **reuses** the cache across loads. **Video and audio are consumed, not generated**: those file types ask the same provider (so a first-frame or embedded-cover thumbnail that the system — or a future Celestina media app — has cached shows automatically, a video frame carrying a small play badge), but Siderita never decodes them itself — it shows a themed `video-x-generic` / `audio-x-generic` icon until the cache has one. Generating video/audio thumbnails belongs to a **separate media project** (it would pull ffmpeg / cover-art readers into the closure) — Siderita stays the lean consumer. The **spacebar quick-look** is in: pressing space previews the selected entry without opening an app — images render full-size (a capped, EXIF-aware decode straight from the file), plain text and code show in a monospace pane (a bounded, binary-safe 128 KiB read via the controller's `preview_text`), folders and binaries get an info card, and ↑/↓ browse the folder live while the overlay stays open (Space / Esc / click-outside dismiss; focus returns to the view on close). Video and audio previews are deliberately left to **Fluorita**, the media app (they need the media decode stack Siderita won't carry) — the info card names those two kinds, and a live "trailer" preview will later embed Fluorita's widget. PDF remains a generic unsupported preview and is not part of Fluorita's current contract
      This records the shipped CP3 behaviour. S6 replaces the text branch with
      Grafita; S7 separately replaces image/video/audio with Fluorita. Folder,
      binary and unsupported branches stay Quick Look.
- [x] Per-item appearance — an entry's context menu offers "Cambiar icono…" plus a closed, named accent palette; shape and colour can be restored independently and follow the entry through the file view, quick look and properties. Both persist atomically in the backward-compatible `path\ticon\taccent-key` records of `~/.config/siderita/icons.conf` and ride one QML appearance property, so readers never observe fields from different updates
- [x] "Open terminal here" — launching the desktop's terminal, not an embedded one — the folder menu's "Abrir terminal aquí" spawns an external terminal with its working directory set to the current folder, honouring `$TERMINAL` then a list of common emulators (foot/alacritty/kitty/wezterm/gnome-terminal/konsole/xfce4-terminal/xterm), the first installed one winning; detached and reaped, with a truthful `op_error` if none is found. No embedded terminal — the CP3 boundary holds

## Checkpoint 4 — A manager that holds the user's arrangement (S4)
**Goal:** the manager stops being the same for everyone. CP3 added the comforts
a file manager is expected to have; CP4 is about the parts the user arranges —
the sidebar, the order of a listing, how a folder is shown, what a session
reopens as — plus the drag and paste refinements those daily habits expose. Same
rule as CP3: each item earned by a demonstrated need, never by parity.

- [x] Natural, case-insensitive name order — the listing compared raw bytes, so `Zebra` sorted before `apple` and `file10` before `file2`. `siderita-core` gained a `compare_names` used by both the scan and the projection: ASCII-case-insensitive, with runs of digits compared as numbers (no parsing, so no overflow however long the run), falling back to the raw bytes so names differing only by case or by leading zeros keep a **total, stable** order. Deliberately not full Unicode collation — that means an ICU-sized table the allowlist does not admit — so non-ASCII bytes still compare as themselves and never panic. 7 unit tests, including non-UTF-8 names
- [x] Favourites — an entry's context menu stars it ("Añadir a favoritos"), the star is drawn as a badge in the corner of its tile in list and grid, and the starred paths get their own **sidebar section**: a starred folder opens, a starred file reveals itself in its folder (the sidebar never launches an application from a single click), and one whose target is gone is struck through rather than quietly dropped — it is still a mark the user set. Persisted to `~/.config/siderita/favorites.conf`, published as one `path\tkind` property so a reader never sees half an update
- [x] An organizable sidebar — bookmarks and places are both drag-reorderable (the row lifts, a line shows where it lands, the release persists), and places can be hidden and brought back ("Mostrar N ocultos", mirroring what Dispositivos already offered). The controller owns which places exist, in what order, minus the hidden ones; the QML only knows how to draw a key. Since a drag cannot be the only way to do a thing, the bookmark menu also carries Subir / Bajar. Order and hidden set persist in `settings.conf`; a place a later version adds simply appears at the end instead of vanishing
- [x] Per-folder view and sort — one global view mode meant a photo folder and a source tree had to look the same. Arranging a folder (view mode, sort field, direction) now records it for **that** folder in `~/.config/siderita/folder-views.conf`, applied on arrival; the global setting stays the default for folders never arranged, and the folder menu offers "Olvidar la vista de esta carpeta" so the record is reversible. Capped at 250 folders, least-recently-arranged first off the front
- [x] Session — the window reopens at the size it was left, with the tabs that were open and the one that was active (`settings.conf`; only the size, since a Wayland client cannot honestly place its own window). A launch that names a folder is about that folder: the saved session does not talk over it. The window stays hidden until the geometry and tabs are in place, so it never flashes at the default size and jumps
- [x] Drag comforts — a drag that rests on a folder for 800 ms **springs it open**, so a move somewhere deep no longer means dropping the entry halfway and picking it up again; and a drag that reaches the top or bottom edge of the view scrolls it, so the destination does not have to already be on screen
- [x] Batch rename — multi-select and rename existed, but not together. "Renombrar N elementos…" applies a rule (find/replace, or a numbered pattern where `#` is the counter and the extension is kept) and **previews every name before anything is touched**: a name that would collide — with a sibling in the batch or with a file already in the folder — is marked and the rename refuses to run. Each rename is still attempted independently by the domain, which refuses to overwrite, so a failure lands on one entry and is reported rather than silently swallowed. No undo record: a batch rename is many renames, and the single undo slot can only honestly reverse one
- [x] Recientes — the desktop's own `recently-used.xbel`, read (never written) and shown as a content-view location beside Papelera, newest first, entries whose file is gone dropped, capped at 100. A small hand scanner rather than an XML dependency, and interop rather than an index: the applications that open things are what write that file. 4 unit tests
- [x] Per-collision conflict choice — closes CP1's one remaining refinement, above
- [x] Motion — the overlays fade instead of popping, thumbnails fade up as they decode, sidebar and content rows ease between hover / selected / current, and a row dropped where it started eases back rather than snapping. Opacity only on anything wearing glass: a scale or a slide moves the surface, and a moving surface samples the wrong region (the same trap a995619 fell into)
- [x] A tab scans when it is first shown — a restored five-tab session used to fire five directory scans before the first frame. Measured on three tabs over a 10 000-entry folder: **186 → 133 MiB PSS** and a third off the startup CPU, because two of those listings are no longer built for folders nobody is looking at. The session file still records every tab, started or not
- [x] Dropped the dormant Trash overlay — 254 lines of QML superseded when Trash became a content location, still instantiated **per tab**. Removing dead weight is the cheapest optimization there is
- [ ] Real-Wayland validation of CP4 — the drag gestures (spring-open, edge scroll), the sidebar reorder drags, and the look of the new surfaces are input- and pixel-shaped, so they are unvalidated until they are used on the real session. The **menu blur p95 must be re-measured**: menus capture live now, which moves that cost from one snapshot to per-frame while a menu is open

## Checkpoint 5 — The desktop's file chooser (S5)
**Goal:** the dialog every application shows when it asks for a file becomes
Siderita's, over the standard that exists for exactly this, without any of those
applications knowing or changing.

- [x] `org.freedesktop.impl.portal.FileChooser` backend — `OpenFile`, `SaveFile`
      and `SaveFiles` served from the app (`src/portal.rs`, one more zbus
      interface beside `FileManager1`). A backend's method reply *is* the answer,
      so each call is held open on an async await point — not a blocked thread —
      while the user browses, and the connection keeps serving other requests
      meanwhile. Every request also exports an `impl.portal.Request` object so
      the front-end can withdraw a dialog whose application has gone away
- [x] The picker window — a window of its own with its own controller, not a tab
      and not a modal of the main window: several applications can be asking at
      once, and a picker can exist before the main window does. It reuses the
      browsing core but deliberately **not** the browsing chrome — no tabs, no
      drag-and-drop, no write verbs; a dialog that can rename and delete while an
      application waits on it is a dialog that can surprise you. Open, save
      (with a name field) and directory modes; multi-select when the caller asks
      for it. Its header is the same animated breadcrumb/search `TopBar` as the
      main browser (including Ctrl+L and Ctrl+F), while the floating chrome
      leaves the first and last grid rows clear, reuses the main view's
      `Ocultos` control and opacity, distinguishes cells that can be returned
      from folders that can only be traversed, and exposes visible keyboard
      focus plus accessible selection state
- [x] Answered on delivery, not on click — the dialog closes when the backend
      says the reply is on its way back, not when the button is pressed. A
      portal-activated process can otherwise exit before the reply is flushed,
      which is the same "a click is a request, never proof" rule the rest of the
      app follows, applied to its own exit
- [x] `--portal` activation — a `.portal` registration and a D-Bus service file
      (installed by `scripts/run.sh`) so a file dialog works whether or
      not Siderita is running. Verified on the real session: with nothing
      running, a call started `siderita --portal`, mapped a picker window with
      `app_id = org.celestina.Siderita`, and `Close()` withdrew it and answered
      cancelled
- [x] File-type filters — the caller's list drives a compact menu in the picker's
      footer, and matching lives in the core beside the ordering
      (`name_filter.rs`: `*`, `?` and literal text, case-insensitive, iterative
      so a pattern full of stars cannot blow up; 6 unit tests). Two rules make it
      safe rather than merely functional: **folders are never filtered** (a
      filter says what you may pick, not where you may go), and an **unknown MIME
      type widens to `*`** instead of hiding — the portal speaks MIME while a
      listing knows names, so the common types are mapped to extensions in the
      backend and anything unrecognised shows everything. There is always a
      "Todos los archivos" row, so a filter can never trap the user. Verified
      through a real portal call: `image/jpeg` became `*.jpg|*.jpeg`, `scan.JPG`
      matched case-insensitively, and the folder survived the filter
- [x] The picker opens at all — `ScrollBar.vertical.policy` was set on the
      entry `GridView`. That attached property only exists once a scroll bar is
      *assigned*, so reading it gave null, assigning `.policy` to null threw,
      and the throw aborted construction of the whole delegate: the window was
      never built and no file dialog ever appeared in any application. Exactly
      the fault `TabStrip` had, in the one window no smoke test covers, because
      the picker is only instantiated by a real portal request. The grid scrolls
      by wheel and keyboard as the manager's does. Verified on the real session
      with a requester that waits for `Response` (`niri msg windows` showing
      `App ID: org.celestina.Siderita`); `transientParent` was tried and proved
      unnecessary, so it was not kept
- [x] One backend, not a queue of ghosts — the name was requested through
      `Builder::name`, whose documentation says `DoNotQueue` is always set while
      its code passes the flags through untouched, and they default to empty. So
      a second activation was answered `InQueue` rather than `Exists`, zbus did
      not treat that as an error, and the process sat in the name's queue
      forever: never serving, never exiting, and inheriting the name the moment
      the real backend died. Found live with 16 stranded `--portal` processes
      holding 3.5 GiB, the oldest two days old — a leak, and a second backend
      able to silently take over, though the dialog never appearing was the
      `GridView` fault above rather than this one. The name is now requested
      after `build()` with an explicit `DoNotQueue`, and a process that cannot
      serve reports it (`backendUnavailable`) and quits — but only when it was
      activated to be the backend, so a Siderita the user opened stays open and
      merely goes without it. Verified on a private bus with no service
      directory: the second process exits, the first keeps the name, and a
      normal launch survives
- [ ] Window parenting — `parent_window` arrives as a `wayland:` handle and is
      currently ignored, so the picker is a free-floating dialog rather than a
      transient child of the asking window. Fixing it needs the `xdg-foreign`
      protocol
- [ ] Daily-use validation — routing is opt-in until it has been lived with
      (`portals.conf`); the failure mode of a bad file chooser is every
      application's upload button, so it earns the switch rather than assuming it

## Checkpoint 6 — Edit text with Grafita (S6)

**Goal:** browsing and small text edits form one continuous interaction without
moving Grafita's document domain into the file manager.

- [x] Add `grafita-core` as the only text classification, document, undo,
      conflict and save implementation; Siderita owns only its adapter and modal
      — `src/editor.rs` is a `GrafitaEditor` QObject of its own that marshals Qt
      types over the shared `grafita-core::session` and copies no domain rule,
      not even the staleness bookkeeping
- [x] Repair the offscreen UI gate that made all of this unverifiable: a dead
      `ScrollBar.horizontal` binding in `TabStrip.qml` (no horizontal scroll bar
      is ever attached to that `ListView`, so both assignments errored and did
      nothing) aborted the component, and since `TabStrip` sits inside
      `FolderView` the failure cascaded to `Main.qml`'s tab delegate — the whole
      folder view never constructed. `scripts/smoke.sh` reported OK throughout
      because it only grepped for `TypeError`/`ReferenceError`; it now also
      fails on construction errors, proven against the unfixed tree
- [x] Route `Space` asynchronously: editable text opens a nearly full-window
      `GrafitaEditorDialog`; S6 itself does not change the image/media/folder/
      binary branches — `FolderActions.requestPreview()` asks Grafita's worker
      and only a decline reaches `QuickLookView`
- [ ] Route double-click and `Enter` on textual content to the standalone
      Grafita application; non-text files keep their existing default handler —
      still `xdg-open`. Grafita now exists, so with it installed the desktop's
      own handler resolution already reaches it; what is missing is Siderita
      *overriding* that with a content probe, so a misnamed file opens in the
      right editor. Both interception points sit exactly on frozen size
      baselines — `src/controller.rs` (1223) owns the bridge and the Rust
      struct, `qml/views/FolderView.qml` (834) owns the only place a signal
      could be handled — so making room means extracting from one of them
      first, a mechanical refactor that should not ride along with a behaviour
      change
- [x] Make the embedded surface a real simple editor — caret, selection,
      undo/redo, save, dirty/conflict/error state and Guardar/Descartar/Cancelar
      on close — without tabs, settings or project chrome. Undo, redo and save
      are intercepted before the widget's own text history, which knows nothing
      about savepoints or terminators. **Built, not yet tried in a session.**
- [x] Remove the synchronous `preview_text` decision from the editable-text
      path; bounded owned workers apply only generation/revision-current results.
      `preview_text` survives only as the read-only quick-look renderer for
      content the core already refused as editable
- [x] Verify content rather than suffix: `.txt`, source, JSON, KDL, dotfile and
      extensionless fixtures take the text routes, while a binary fixture does
      not — covered by `grafita-core`'s integration fixtures, which are the same
      classification this adapter calls
- [ ] Validate build, smoke, keyboard/focus, reduced motion and the modal on a
      real Wayland session; an offscreen start is not interaction evidence.
      Guard, fmt, Clippy, the workspace tests, `qmllint` against the current
      build's module, a release build and `scripts/smoke.sh` all pass; typing,
      selection, focus trapping, focus restoration, reduced motion, IME and
      closing with a dirty document remain unverified in a compositor. A
      temporary offscreen probe did drive the real modal end to end — binary
      declines to quick look, text opens, focus lands in the editor body, typing
      through Qt's own `TextEdit` marks it dirty, saving writes
      `primera\r\nsegunda EDITADA\r\ntercera\r\n` with its CRLF intact,
      closing while dirty raises the guarded question, and undo/redo walk the
      savepoint correctly — so what remains is specifically the compositor and
      input stack: synthesised key events for Escape/Ctrl+S/Ctrl+Z, Tab focus
      trapping, focus restoration, reduced motion, IME, AT-SPI and glass

**Done when:** `Space` edits and saves a textual file inside Siderita, while
double-click/Enter opens the same file in standalone Grafita, and both paths
derive their document truth from `grafita-core`.

## Checkpoint 7 — Play media with Fluorita (S7)

**Goal:** images, video and music can be viewed or played from Siderita without
duplicating Fluorita's library/playback domain or paying decode cost while
ordinary folders are merely visible.

- [ ] Consume `fluorita-core` and the lazily loaded `fluorita-engine`; Siderita
      owns only its adapter and minimal-player composition
- [ ] Route `Space` on image/video/audio to a minimal Fluorita modal; text stays
      Grafita's branch and folders/binaries/unsupported content stay Quick Look
- [ ] Route double-click and `Enter` on media to standalone Fluorita and begin
      that item in the full Gallery/Music application
- [ ] Keep list/grid artwork cache-only: image thumbnail, video poster and audio
      cover are static PNGs; short video trailers run only on explicit demand,
      one per host, and cancel on selection change
- [ ] Expose only supported controls — view for images; play/pause, seek and
      volume for video/audio — with truthful pending/confirmed/error state
- [ ] Verify the engine is not loaded during normal browsing, worker/session
      shutdown is deterministic, and stale artwork/trailer/playback events never
      publish after a newer selection
- [ ] Validate real image/video/audio, keyboard/focus, reduced motion, frame
      pacing and resource budgets on Wayland; offscreen startup is not playback
      evidence

**Done when:** `Space` uses the minimal player for real local media and
double-click/Enter starts the same item in full Fluorita, while ordinary folder
browsing still consumes only cached static artwork.

## Non-goals

No cloud/network, global indexer, archive VFS, plugins, IDE, terminal or suite
daemon. S6 and S7 are bounded consumer surfaces over the Grafita and Fluorita
contracts, not independent editor/player domains or an exception that permits
other applications to accumulate in Siderita.

The boundary, not a loophole: CP3's bounded non-indexed filename search is not the
global indexer ruled out here, and "open terminal here" launches an external
terminal rather than embedding one. Everything else on this list stays out until
the plain local manager (CP0–CP2) is complete and a daily need is shown.
