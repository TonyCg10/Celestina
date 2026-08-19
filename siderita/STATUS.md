# Siderita status

- **Updated:** 2026-08-18
- **Implementation:** the registered product version and CP0-CP7 behaviour are
  present; `SID-G7` (shared reading surface) and `SID-A1` (archives) are the
  active checkpoints and the portal-parenting one remains planned
- **Author validation:** mixed; current manual queue is in
  [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- Uncommitted in the checkout: `SID-A1`, compressing and extracting. A new pure
  crate, `siderita-archive`, identifies a container by its bytes, lists it,
  extracts it into a folder and creates one, holding `siderita-ops`' own
  guarantees: it never overwrites (a second extraction lands beside the first
  under a freed name), it stages the whole extraction and promotes or removes
  it whole, and it refuses any member whose stored name or symlink target would
  land outside the destination — the stored bytes are read, never `zip`'s
  sanitised spelling, so an escape is reported instead of quietly renamed. The
  containers are pure Rust: no `unzip`, no `tar`, no process. The entry menu
  gains the extract verb — offered only when the domain says every selected entry
  really is an archive — and «Comprimir…», which asks for a name and a container
  (ZIP or TAR.GZ) and suggests a free one. Both run on the paste operation's
  worker, progress and Cancel button. `controller.rs` did not grow: `UndoAction`
  and `ConflictStrategy` moved out to `controller/actions.rs` and the ratchet
  fell to 1092.

  Exercised against real archives rather than only fixtures, which is what
  found the rest: a 12 MB zip from the cache extracts byte-identical to
  `unzip`; archives this domain writes pass `unzip -t` and `tar tzf` and come
  back identical through both tools; a hand-built zip-slip, an absolute member
  and an escaping symlink are each refused with the destination left empty; a
  truncated zip reports damage. Three defects that only a real archive shows
  were fixed on the way — `next_available` cut `web-2.1.2` into
  `web-2.1 (copia).2` (a folder name is not `stem.ext`, and the same defect hit
  copying folders, so it was fixed in its owner and now takes a `NameShape`),
  modification dates were dropped on both write and read, and the archive
  refusals reached the person in English. Formatted, Clippy-clean, 14 domain
  tests, 31 ops tests, 109 unit tests, 54 QML tests, the offscreen smoke and
  the three repository guards — no production run, no version transition, no
  inventory and nothing tried by hand: that is `VAL-SID-07`.

- Uncommitted in the checkout: `SID-G7-I`. The tab strip keeps deriving its own
  label: routing it through the adapter costs qmllint warnings the project's
  inventoried debt ceiling refuses, and a ratchet is not raised for a label. The
  folder heading, where it cost nothing, keeps the adapter.

- Uncommitted in the checkout: `SID-G7-H`, two of the low findings of the
  [light monorepo audit](../docs/evidence/2026-08-06-light-monorepo-audit.md).
  Pressing Ctrl+V with nothing on the clipboard but a cut into the folder those
  entries already occupy is no longer a silent no-op: the plan reports the
  entries it drops instead of forgetting them, so the clipboard and its ghost
  are settled — the system clipboard only while it still holds exactly those
  entries — and the status line says why nothing moved. And the last two QML
  surfaces that cut a label out of a path, the tab chip and the folder heading,
  ask the adapter for it, which is what
  [ADR 0008](../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md) says
  owns that answer; the chip gains the phone-name substitution it never made.
  One visible consequence: at the filesystem root the chip now reads `/` rather
  than `Inicio`, matching the heading beside it. Formatted, Clippy-clean, 109
  unit tests and the three repository guards — no production run, no version
  transition, no inventory and nothing tried by hand: that is `VAL-SID-06`. The
  limits, including one comparison against a key left deliberately open, are in
  the [evidence](docs/evidence/2026-08-06-silent-paste-and-label-surgery.md).

- Uncommitted in the checkout: `SID-G7-G`, the three Siderita items of stage 3
  of the [light monorepo audit](../docs/evidence/2026-08-06-light-monorepo-audit.md).
  A breadcrumb is published key-first, so a folder whose name contains a tab — a
  legal filename character — no longer moves the cut QML makes and leaves the
  crumb holding a fragment instead of a key. A persisted path record is now
  *marked* as a key when it is written (`key:`) rather than recognised by
  re-encoding it and hoping the codec is idempotent, which could not tell a
  pre-ADR raw path holding a literal `%20` from the key for a path holding a
  space and silently answered with the second — for a bookmark, which is a
  navigation and a paste target, that is the wrong folder. And the send-to-phone
  menu item calls Magnetita's new `SendFileUri` with the byte-exact `file://` URI
  `dbus::path_to_uri` already writes, closing the last verb that put a lossy
  path out of the process; `SendFile` is untouched, because it is a published
  interface with other possible callers. Formatted, Clippy-clean, 104 unit tests
  and both repository guards — no production run, no version transition, no
  inventory and nothing tried by hand: that is `VAL-SID-06`. A record written
  before the mark keeps the old ambiguity until its store is saved once; that
  limit and the rest are in the
  [evidence](docs/evidence/2026-08-06-path-key-correctness-debt.md).

- Uncommitted in the checkout: `SID-G7-F`, repairing a regression `SID-G7-E`
  introduced. The thumbnail provider converts its id with `toUtf8`, so a name
  carrying an accent resolves again; the seam is now exposed and tested through
  the same URL a delegate writes, which is where the previous tests were not
  looking.

- Uncommitted in the checkout: `SID-G7-D`, the byte-exact path seam. Every path
  that crosses the Qt boundary is now the percent key of
  [ADR 0008](../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md),
  published beside its own lossy display text; every invokable decodes that key
  and refuses a malformed one with a typed error instead of rebuilding a
  `PathBuf` from the `QString`; and QML no longer composes paths — the
  breadcrumbs, the save picker's typed name, the quick look's `file://` URL and
  the sidebar's derived names all come from the adapter. The persisted
  bookmarks, favourites, icons, folder views and tab session migrate to keys on
  load. The `file://`, portal and Trash encodings that face other processes are
  untouched. This closes audit finding `SID-A2` in the checkout; `FLU-M1`, the
  same defect in Fluorita, is untouched. Compiled, unit-tested, QML Test 47/47
  and an offscreen smoke — no production run, no version transition, and nothing
  tried by hand on a real session: that is `VAL-SID-06`. The two limits it
  recorded in its
  [evidence](docs/evidence/2026-08-06-byte-exact-path-seam.md) are closed by
  `SID-G7-E` below and are no longer open: a non-UTF-8 name used to get a
  generic glyph instead of a thumbnail, because the C++ provider
  addressed files through `QString`; and the system clipboard used to exchange
  paths with the rest of the desktop, so copying such a name *to another
  application* was lossy.
- Uncommitted in the checkout: `SID-G7-E`, the two remaining Qt seams that still
  decoded a byte-exact key into a `QString`. The thumbnail provider now carries
  the decoded path as `QByteArray`, finds the file with `::stat` on those bytes,
  reads it through a descriptor opened on them, derives the extension from them,
  and computes the freedesktop cache key over them in the exact spelling
  `celestina_core::percent::encode_qt_path` owns — a test asserts the C++ and the
  Rust agree byte for byte, and another decodes a real 2x2 PNG named with
  `b"na\xffme.png"` out of a temporary directory. The system clipboard stops
  exchanging paths with the rest of the desktop and exchanges the
  percent-encoded `file://` URIs it actually speaks, written and read by
  `dbus::path_to_uri` and `dbus::uri_to_path`, so a non-UTF-8 name survives a
  copy to another application and back; `holds_exactly` still compares real
  `PathBuf`s. Compiled, `cargo fmt`/Clippy clean, 93 unit tests, both repository
  guards and `qmllint` unchanged — no production run, no version transition, no
  inventory and nothing tried by hand: that is still `VAL-SID-06`. What remains
  is in the
  [evidence](docs/evidence/2026-08-06-thumbnail-and-clipboard-bytes.md).
- Uncommitted in the checkout: `SID-G7-C`, the corrective unit from the suite
  audit. Pasting an entry into its own folder now duplicates instead of trashing
  the original to make room for it; the portal answers `writable` only when it
  was asked for and confirms an overwrite before returning a save destination;
  trash takes the running-operation guard paste already had; a vanished entry no
  longer fails a listing and a quiet refresh stays quiet; a symlink to a
  directory is navigable; trash entries are purged by info path rather than list
  position; dropped URIs are decoded by bytes in Rust; and the four remaining
  configuration files are written atomically. Compiled and unit-tested, with no
  inventory, no version transition and no production run — the author asked for
  the corrections, not the delivery. Nothing here has been seen by a real portal
  requester: that is `VAL-SID-05`.
- The embedded Grafita editor and the quick look's text pane number their
  lines and scroll with the suite's shared `CelestinaLineGutter` and
  `CelestinaScrollBar`; the editor reports the caret's line and character
  column and no longer shows an encoding label. Both consume the same
  `grafita-core` mapping Grafita's own window does.
- The Rust/CXX-Qt/QML implementation is the only Siderita application. It
  navigates, filters, sorts, searches, watches, tabs and performs loss-free file
  operations through the shared Siderita crates.
- The main window includes places, bookmarks, favourites, removable volumes,
  Magnetita phone state, per-folder views, recent files, thumbnails, properties,
  Trash and batch operations.
- The portal backend and picker implement open/save/directory modes, filters and
  on-demand D-Bus activation. The incoming Wayland `parent_window` handle is
  imported through `xdg-foreign` (`cpp/windowparent.cpp`, generated from
  wayland-protocols at build time), so the picker asks the compositor to treat
  it as a child of the window that requested it. The handle is fetched by token
  rather than carried in the request signal. Whether a given compositor honours
  the relationship is its own decision; without `zxdg_importer_v2`, or with a
  caller that sent no window, the dialog is simply centred and unparented.
- The picker now opens as a dialog rather than a second file manager: 780x560
  centred on the active screen, one compact row per entry (small icon, name with
  its size beneath, date to the right) instead of the thumbnail grid, and a
  narrower places panel that survives down to 560 px of width. Its selection
  band, keyboard traversal and read-only policy are unchanged — the view is a
  one-column grid, not a new list.
- Grafita integration is complete in the checkout: `Space` opens the editable
  modal and content-based double-click/`Enter` launches standalone Grafita.
- Fluorita integration is complete in the checkout: `Space` opens the minimal
  image/video/audio surface and activation launches standalone Fluorita. Normal
  browsing consumes only cached static artwork.
- Folder and file-type icons both use the current filled CelestinaStyle content
  components. Older statements that file types remain flat are historical.

## Planned implementation debt

- Add the narrowest automated lifecycle/handle tests around the portal request
  and its parenting (`SID-M1`). The parenting itself is in the checkout; what
  is missing is coverage that a withdrawn request drops its stored handle and
  that a second adopt call cannot leave an import behind.
- After the shared style motion inventory exists, remove any remaining local
  Siderita motion gaps and extend automated focus/event coverage where possible.
- Frozen large-file baselines may only shrink; they are not permission to place
  new behaviour in the coordinators.

## Blockers

There is no implementation blocker recorded. Real drag, blur, portal daily-use,
reduced-motion and assistive-technology checks are independent validation work,
not blockers for already completed checkpoints.

## Evidence boundary

The detailed CP0-CP7 record and historical commands are in the
[archived roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the exact canonical release passed the app and selected workspace Rust matrices,
QML Test 47/47 and an eight-second smoke; `qmllint` completed with 326 existing
non-fatal baseline warnings. See the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). No portal route
or installed binary was changed.

On 2026-08-05 the picker's dialog shape was built in release, passed QML Test
47/47, and answered an `OpenFile` request from a private `dbus-run-session` bus
under `QT_QPA_PLATFORM=offscreen` with no QML diagnostics. At the author's
request `scripts/run.sh` then installed that build into `~/.local`, so the
session's file chooser is this tree. The portal route in
`~/.config/xdg-desktop-portal` was not changed. Appearance on the real
compositor is `VAL-SID-02`, still pending.

The `xdg-foreign` parenting landed the same day and was installed too. What was
verified automatically: it builds and links, `cargo fmt`/Clippy are clean, QML
Test is 47/47, the session's compositor advertises `zxdg_importer_v2`, and an
offscreen portal request carrying `wayland:<handle>` reaches the picker with
that handle and declines to adopt without crashing. What was **not** verified:
that a real compositor stacks the dialog over the application that asked — that
needs two real windows and belongs to `VAL-SID-02`.

## Records

- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Content activation contract](../docs/contracts/content-activation.md)
- [Registry entry](../docs/projects.toml)
