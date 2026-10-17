# Celestina status

- **Updated:** 2026-10-16
- **Version:** the one `celestina/Cargo.toml` carries, set by each landing
- **Implementation:** the CEL-1 program is landed on `main` through CEL-1-H
  (1.0.0), the tokens as data (CEL-1-C) included: the three toolkit crates
  (`celestina-wayland`, `celestina-paint`, `celestina-ui`); one Island capsule
  per output that grows into the Activity, Notifications and Tray pages, with
  page tabs and the overflow popup; the launcher with IME and paste; the
  notification server with banners, history, actions and do-not-disturb; and
  the tray with nested DBusMenu popups. The shell claims
  `org.freedesktop.Notifications` and `org.kde.StatusNotifierWatcher` only
  when the name is free. Evidence per unit is in
  [`docs/evidence/`](docs/evidence/README.md). The halted Qt/QML shell this
  project replaces lives at [`celestina-qt/`](../celestina-qt/README.md).
- **Exit (CEL-1-H):** landed on `main` as celestina 1.0.0 (`00c6d0bb`), kind
  `release`: the live rows queued in [VALIDATION.md](VALIDATION.md), the
  [handover note](docs/handover.md) on what Noctalia still does for the
  session, and the documents up to date
  ([evidence](docs/evidence/2026-10-16-exit.md)). The author chose 1.0.0 as the
  first stable version, not the definitive one. The CEL-1 plan was archived on 2026-10-16 by the unit that opened CEL-2.
- **Focus:** CEL-2, the parity program
  ([plan](docs/plans/active/2026-10-16-cel-2-parity.md)): the bar and the piece
  contract first, with CEL-2-B and CEL-2-E under way beside it; the author's
  live validation of the nine VAL-CEL rows stays queued.
- **CEL-2-E (in progress, not landed):** the `wallpaper` duty, off by default,
  on `unit/celestina/CEL-2-E`: one still image per output on a
  `celestina-wallpaper` Background layer surface, bounded decoding, and owner
  detection so an output another client holds is refused. Its evidence is
  [`docs/evidence/2026-10-16-wallpaper.md`](docs/evidence/2026-10-16-wallpaper.md);
  its author row is `VAL-CEL2-WALLPAPER`, pending.
- **Author validation:** all nine VAL-CEL rows are `pending`; the author has
  not recorded a result in any of them. The author accepted 1.0.0 as stable
  after trying the shell, without closing the rows; the recorded evidence is
  unit tests, a private bus and a smoke over a nested niri.
- **Blockers:** none.
