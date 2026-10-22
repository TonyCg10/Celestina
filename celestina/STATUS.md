# Celestina status

- **Updated:** 2026-10-17
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
- **Energy (CEL-2-B):** landed with this unit: the idle chain on
  `ext-idle-notify`, caffeine as a logind inhibitor, the forced night light on
  `wlr-gamma-control` and the `session` verbs, each duty off by default behind
  `duties.idle`, `duties.caffeine` and `duties.nightlight` in `shell.json`
  ([evidence](docs/evidence/2026-10-16-energy.md)). The four VAL-CEL2 rows are
  `pending`; the version is set by the landing.
- **OSD (CEL-2-C):** in progress, not landed: the `osd` duty, off by default
  behind `duties.osd` in `shell.json`: a `celestina-osd` overlay surface for
  volume, microphone and brightness changes (audio through `pactl subscribe`
  and `wpctl`, brightness through sysfs and a lazy, leased `ddcutil` thread),
  the `volume`, `microphone` and `brightness` verbs, and the `volume` and
  `brightness` bar pieces with a slider popup
  ([evidence](docs/evidence/2026-10-17-osd.md)). The three VAL-CEL2-OSD rows are
  `pending`; the lock-keys OSD is not delivered.
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
- **CEL-2-K (in progress, not landed):** the glance pieces on
  `unit/celestina/CEL-2-K`: `sysmon` (CPU and memory through
  `hematita-core`), `weather` (Open-Meteo, only with a location written in
  `shell.json`) and `clock` (the short date), none in the default zones, plus
  the Sistema and Reloj pages of the Island. No location, no request
  ([ADR 0016](docs/decisions/0016-weather-is-the-only-network-and-manual.md)).
  Its evidence is
  [`docs/evidence/2026-10-17-glance.md`](docs/evidence/2026-10-17-glance.md);
  its author rows are `VAL-CEL2-SYSMON`, `VAL-CEL2-WEATHER`,
  `VAL-CEL2-CALENDAR` and `VAL-CEL2-GLANCE-TABS`, pending. Review pending.
- **CEL-2-J (implemented in its worktree, not landed):** the `phone` and
  `bubbles` duties, both off by default: the Magnetita phone piece and its
  Island page, and the minimised-window bubbles with the `Mod+K` selector over
  Melibea's socket. Its evidence is
  [`docs/evidence/2026-10-17-phone-and-bubbles.md`](docs/evidence/2026-10-17-phone-and-bubbles.md);
  its author rows `VAL-CEL2-PHONE`, `VAL-CEL2-BUBBLES` and
  `VAL-CEL2-BUBBLES-KEYS` are pending.
- **CEL-2-D (in progress, not landed):** the `media` duty, off by default: an
  MPRIS client on its own bus connection, the media piece, and the Medios page
  with a session-only pin. Its evidence is
  [`docs/evidence/2026-10-17-media.md`](docs/evidence/2026-10-17-media.md); its
  author rows are `VAL-CEL2-MEDIA` and `VAL-CEL2-MEDIA-COVER`, pending.
- **Author validation:** all nine VAL-CEL rows are `pending`; the author has
  not recorded a result in any of them. The author accepted 1.0.0 as stable
  after trying the shell, without closing the rows; the recorded evidence is
  unit tests, a private bus and a smoke over a nested niri.
- **Blockers:** none.
- **In progress (CEL-2-A, not landed):** the bar and the piece table on
  `unit/celestina/CEL-2-A`; evidence in
  [`docs/evidence/2026-10-16-bar-and-pieces.md`](docs/evidence/2026-10-16-bar-and-pieces.md)
  records the checks, the tests and the nested smoke green on the tree after
  the review fix pass, and the gaps the unit leaves open.
  `VAL-CEL2-BAR-LIVE` is pending.
- **In progress (CEL-2-G, not landed):** the `clipboard` duty, off by default:
  an `ext-data-control` history of text and PNG entries that skips selections
  carrying the password-manager hint, keeps text only in a 0600 state file
  deleted when the duty goes off, and offers a bar piece, an Island page and
  verbs that never return content. Evidence in
  [`docs/evidence/2026-10-17-clipboard.md`](docs/evidence/2026-10-17-clipboard.md);
  its author rows `VAL-CEL2-CLIPBOARD-LIVE`, `-SECRET`, `-IMAGE` and `-LOOK` are pending.
- **In progress (CEL-2-F, not landed):** the network, Bluetooth, battery and
  power-profile pieces, read-only state from NetworkManager, BlueZ, UPower and
  power-profiles-daemon with a closed list of actions, each popup an Island page;
  evidence in [`docs/evidence/2026-10-17-connectivity.md`](docs/evidence/2026-10-17-connectivity.md);
  `VAL-CEL2-NETWORK`, `VAL-CEL2-BLUETOOTH` and `VAL-CEL2-POWER` are pending.
