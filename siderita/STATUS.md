# Siderita status

- **Updated:** 2026-08-04
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
  still ignored, so the picker is not yet a transient child.
- Grafita integration is complete in the checkout: `Space` opens the editable
  modal and content-based double-click/`Enter` launches standalone Grafita.
- Fluorita integration is complete in the checkout: `Space` opens the minimal
  image/video/audio surface and activation launches standalone Fluorita. Normal
  browsing consumes only cached static artwork.
- Folder and file-type icons both use the current filled CelestinaStyle content
  components. Older statements that file types remain flat are historical.

## Planned implementation debt

- Implement `xdg-foreign` parenting for portal request handles and add the
  narrowest automated lifecycle/handle tests (`SID-M1`).
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

## Records

- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Content activation contract](../docs/contracts/content-activation.md)
- [Registry entry](../docs/projects.toml)
