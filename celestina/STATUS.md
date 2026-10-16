# Celestina status

- **Updated:** 2026-10-16
- **Version:** the one `celestina/Cargo.toml` carries, set by each landing
- **Implementation:** CEL-1-B, the skeleton, is landed on `main`: the three toolkit crates (`celestina-wayland`,
  `celestina-paint`, `celestina-ui`), one Island capsule per output with the
  clock and the workspace dots over the tolerant niri stream, the growth into
  a page, the `org.celestina.Shell1` command surface with single instance,
  and the production scripts with a nested-niri smoke
  ([evidence](docs/evidence/2026-10-14-skeleton.md)). The halted Qt/QML
  shell this project replaces lives at
  [`celestina-qt/`](../celestina-qt/README.md).
  CEL-1-D, the Island pages, is also implemented on its unit branch: Activity,
  Notifications and Tray, the page tabs with the slide, and the overflow menu
  as an `xdg_popup`, and it awaits landing after CEL-1-C
  ([evidence](docs/evidence/2026-10-16-island-pages.md)).
- **Focus:** landing CEL-1-D after CEL-1-C, then CEL-1-E, CEL-1-F and CEL-1-G of
  the [CEL-1 plan](docs/plans/active/2026-10-14-cel-1-island.md).
- **Author validation:** VAL-CEL-GLASS, VAL-CEL-REST and VAL-CEL-SCALE in
  [VALIDATION.md](VALIDATION.md) can run on the binary B's landing deploys;
  VAL-CEL-ISLAND, VAL-CEL-POPUP and VAL-CEL-KEYBOARD gain their CEL-1-D
  procedures on D's; the other rows wait for their units.
- **Blockers:** none.
