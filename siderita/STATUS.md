# Siderita status

- **Updated:** 2026-11-25
- **Implementation:** the registered product version and CP0-CP7 behaviour are
  present; `SID-A4`, `SID-B1`, `SID-U1`, `SID-U1-B` and `SID-U1-C` are closed and archived;
  `SID-H1`, the hardening that follows the 2026-09-26 monorepo audit, was
  closed on 2026-11-25; its
  [plan](docs/plans/archive/2026-09-26-hardening.md) is archived, and its one
  unstarted row, `SID-H1-D`, is now planned debt (below). No checkpoint is
  active
- **Author validation:** mixed; current manual queue is in
  [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- Version 1.12.0 in the checkout (the last delivery, `SID-H1-S`). The
  delivered and committed units, one bullet each, are in
  [the status history](docs/history/status-through-2026-11-25.md).
- Known and unfixed, recorded with their units and kept in full in the
  history: the shared `CelestinaInputShield` under every modal card takes
  the pointer from a text field, so drag selection still fails in the
  create, format, password, compress and batch-rename dialogs until a
  `celestina-style` unit fixes it (`SID-H1-S`, `VAL-SID-19`); `bytesText` is
  a third copy of the byte formatter; `gtk-launch` fails asynchronously, so
  a missing Hematita is not detected; after a properties takeover the quick
  look shows the glyph until its entry changes (`SID-U1`, `VAL-SID-U1`); and
  the rows slide with the content frame as the heading folds (`1.6.1`).
- Suite conventions ([ADR 0012](../docs/decisions/0012-suite-conventions.md),
  `CONV-1`): Siderita serves `org.celestina.Siderita` through
  `celestina_core::activation` (claim first, `Open(as paths)` with byte-exact
  path keys) and is the suite's launcher: «Abrir en» and a second launch reach
  a running Grafita, Fluorita or Hematita over `org.celestina.Application1`
  and spawn only when nobody owns the name, and its send-to-phone entry is an
  explicit send through Magnetita. Its entry drag carries the `text/uri-list`
  the other applications read. Every window follows
  `~/.config/celestina/appearance.toml` (reduced motion and the text scale on
  the font tokens); Siderita's own text scales are kept until a later unit
  decides them. `VAL-SID-SEND` is the author's check of the send.
- `CONV-1-D` (suite): an entry's context menu offers «Abrir en» — Grafita,
  Fluorita, Hematita when installed and applicable to the acting entries, and
  one send-to-phone entry per connected phone — decided from the folder model
  and by name on the Qt thread, opened through `celestina_core::activation`
  on a worker, and sent through Magnetita's `SendFileUri` on the device
  model's worker ([evidence](../docs/evidence/2026-10-09-open-with.md)).
  `EXT-1-C` (suite) added Calcita for PDFs, decided by name like media
  ([evidence](../docs/evidence/2026-10-09-reading-and-capture-exit.md)).
- `CONV-1-A` (suite): Siderita keeps to one window. A launch claims
  `org.celestina.Siderita` through `celestina_core::activation` before any
  window exists; a later `siderita RUTA…` hands its paths over
  `org.celestina.Application1.Open`, where a folder opens a tab of its own and
  a file a tab on its folder with the file selected, and a bare launch raises
  the window. A `--portal` or `--file-manager` process never claims it, and
  `FileManager1` and the portal are unchanged. Opening a file in Grafita or
  Fluorita, and a folder in Hematita, goes to the running instance through
  the bus on a worker, and spawns the program only when nobody owns its name
  ([evidence](../docs/evidence/2026-10-09-shared-activation.md)).
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

- `SID-H1-D` (SID-20, ruling R-A17; moved here from the archived SID-H1 plan
  on 2026-11-25, waiting on a `celestina-core` local-time owner): Write
  `DeletionDate` in local time, as the spec, GIO and KIO do, through a
  local-time facility owned by `celestina-core` (with `siderita-archive`'s
  `Zone` moved onto it), and order the Trash listing by instant rather than by
  a string that mixes zones. Depends on that `celestina-core` owner; a
  cross-prefix change, so it may land as a `suite:` unit. (SID-20; ruling
  R-A17) Its exit: `cargo test -p siderita-ops --offline`: a record written at
  a known instant and zone reads back local; mixed UTC and local records sort
  by instant. Files: `celestina-rs/crates/siderita-ops/src/trash.rs`;
  `celestina-rs/crates/siderita-ops/src/trashinfo.rs`; the `trash` consumers
  in Siderita, Fluorita and Hematita.
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
