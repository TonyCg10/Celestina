# Siderita status

- **Updated:** 2026-08-05
- **Implementation:** the registered product version and CP0-CP7 behaviour are
  present; `SID-G7` (shared reading surface) is the active checkpoint and the
  portal-parenting one remains planned
- **Author validation:** mixed; current manual queue is in
  [VALIDATION.md](VALIDATION.md)

## Current checkout truth

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
