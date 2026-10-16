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
  first stable version, not the definitive one. The CEL-1 plan stays in
  `docs/plans/active/` until an administrative unit archives it.
- **Focus:** the author's live validation of the nine VAL-CEL rows, then the
  deliveries the handover note lists before Noctalia can be stopped.
- **Author validation:** all nine VAL-CEL rows are `pending`; the author has
  not recorded a result in any of them. The author accepted 1.0.0 as stable
  after trying the shell, without closing the rows; the recorded evidence is
  unit tests, a private bus and a smoke over a nested niri.
- **Blockers:** none.
