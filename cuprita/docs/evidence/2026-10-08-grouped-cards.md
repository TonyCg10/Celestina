# The grouped-card layout — CUP-1-H

- **Date:** 2026-10-08
- **Scope:** `CUP-1-H` of
  [`../plans/active/2026-10-08-cup-1-foundation.md`](../plans/active/2026-10-08-cup-1-foundation.md):
  `celestina-rs/crates/cuprita-core/src/` (`model.rs`, `fake.rs`,
  `nm/mod.rs`), `celestina-rs/crates/cuprita-core/tests/` (`fakes.rs`,
  `order.rs`), `cuprita/src/controller/network_model.rs`, `cuprita/build.rs`,
  `cuprita/qml/pages/` (all three), `cuprita/qml/components/` (new
  `SectionCard`, `PageScroll`, `RowFilter`, `RowDivider`, `EmptyLine`,
  `ConnectionRow`, `SignalBars`; changed `NetworkRow`, `DeviceRow`,
  `StreamRow`, `SettingRow`, `EndpointCard`),
  `cuprita/tests/qml/tst_network_page.qml`,
  `cuprita/tests/qml/fakes/FakeNetworkController.qml`, `cuprita/STATUS.md`,
  the ledger
- **Environment:** Qt 6.11.2, cxx-qt 0.9.1, zbus 5, `qmltestrunner`
  offscreen
- **Artifact:** the release binary in the shared Cargo target, not installed
- **Change kind:** maintenance (1.0.1 stays; no history row)

## Procedure

The author's verdict on 1.0.1: the content sat too close to the edges and
the pages read as things put one after another. The approved design regroups
the three pages in the grammar of Hematita's `ListSection` and Siderita's
rows. This unit builds on the session commit `5fb3fe5c`, which had inset the
setting rows, the search button and the endpoint cards.

| # | Requirement | What this unit does |
| --- | --- | --- |
| 1 | A section label above every card; `Grouped` cards `spaceCardGap` apart; the page scrolls as one | `components/SectionCard.qml` (label, card, a row column inset `spaceCardInset`) is used by every card of the three pages; `components/PageScroll.qml` stacks the cards `spaceCardGap` apart in one `Flickable` with `CelestinaWheelScroll` and `CelestinaScrollBar`. Lists inside cards are non-interactive and as tall as their rows; `PageScroll.reveal` keeps the keyboard's current row (and a row whose menu opens from the keyboard) in view |
| 2 | Content 16 px from the card edge; concentric rows | Rows sit `spaceCardInset` (8) inside the `radiusLg` card with a `radiusMd` plate (12 + 8 = 20); each row places its glyph or words and ends its trailing controls a further `spaceLg - spaceCardInset` in, so both are `spaceLg` (16) from the card edge |
| 3 | Two-line rows; trailing icon buttons; row highlight on hover, focus and current | `NetworkRow`, `DeviceRow`, `StreamRow` and `SettingRow` paint a `CelestinaRowHighlight` (`Content` family, `radiusMd`) driven by a `HoverHandler` and by the list's current-and-focused state; the trailing actions stay `CelestinaIconButton`s |
| 4 | Hairlines between setting rows | `components/RowDivider.qml`: `CelestinaTheme.divider` at `borderHairline`, spanning the words (16 px in from both card edges), the pattern Hematita's services page uses. Shown between Wi-Fi and airplane, the adapter switch and the search, the endpoint selector and volume, and between VPN rows |
| 5 | Red: a connection card with kind, name, kind and IPv4 address and a disconnect button; a muted line when there is none | `Network` gains `address: Option<String>`. `nm` reads the device's `Ip4Config` → `IP4Config.AddressData[0].address` for a connected wire and for the radio carrying the connected Wi-Fi (`first_address`, unit-tested); the fake's wired link has `192.168.1.23` (dropped on disconnect or forget); the model has an `address` role (empty string when none) and the QML stand-in mirrors `fake.rs`. `ConnectionRow.qml` is the large row (`rowHeightLg`, accent glyph, the kind and the address joined by a middle dot, a Ghost disconnect button) |
| 6 | Red: a Wi-Fi card with the two switches | Two `SettingRow`s with a hairline |
| 7 | Red: the networks card without the active row; signal bars; security line; join and forget | `components/RowFilter.qml`, a `DelegateModel` that keeps the rows a predicate accepts, re-run on inserts, removals, moves and `dataChanged`. The model keeps every row (smoke still `networks=4`); the page shows the active row, the other non-VPN rows and the VPNs through three filters |
| 8 | Red: a VPN card only when a VPN exists, one `SettingRow` each | A `Repeater` over the VPN filter; the card hides when it has no row |
| 9 | Bluetooth: an adapter card and a devices card | See decisions; the device list at the new metrics |
| 10 | Audio: labels above, inset, card gap, stream-row highlight | `EndpointCard` is now a `SectionCard` (the output or input label above); the selector row and the volume row inside; the applications and profile cards likewise |
| 11 | Keep keyboard, accessible names, tests | Tab still enters each list once, arrows walk it, Enter runs the primary action, Menu/Shift+F10 opens a device's menu, Esc closes dialogs and menus. A VPN is now a switch row: Space toggles its focused switch, as for every switch. Accessible names of the rows are unchanged; the connection row reads as connected to its name, then its kind and address |

### Design decisions

- **Bluetooth card: two rows.** The adapter switch on one row and, below a
  hairline, the search row: a label for nearby devices (saying it is
  searching while it searches) with the spinner and the search / stop
  button at the right. One row with a switch and a button
  side by side had two trailing controls competing for one label.
- **Signal icon: drawn bars.** `CelestinaIcons` has a single `wifi` glyph with
  no levels, so `SignalBars.qml` draws four rising bars in the glyph ink
  (`textMuted`) in an `iconMd` box; bars above the level take
  `unavailableContentOpacity`. A wired row in the networks card keeps the
  `link` glyph.
- **Which connection is "the" connection.** The first connected non-VPN row
  in model order (the model orders connected first). On the author's machine
  a Wi-Fi and a wire are both connected (see the snapshot below); the first
  goes to the connection card and the second stays in the networks card
  with its leave button.
- **Network secondary line.** The security alone for a Wi-Fi network not in
  use (protected, open, enterprise, or WEP and unsupported); any other state
  is put first (connecting, then the security), so joining and failure stay
  visible. A wire reads its state.
- **Forget stays a trash icon button.** The brief asks for the join and
  forget actions "as today"; today forget is a trash `CelestinaIconButton`,
  not a menu, so it stays one. The spec's per-row menu remains as it was.
- **Empty cards say so.** No connection, no network, no device and no
  application each read as one muted line, instead of an empty card.
- **Audio row labels.** With the card's name above it, the selector row is
  labelled as the default device and the profile row as the card's profile
  on the left, the selector on the right, like every other row's label-and-control
  grammar.
- **Section labels in capitals.** `SectionCard` uppercases the title, as
  Siderita does for its eyebrows; `CelestinaSectionLabel` sets type and ink.
- **Pages without their own surface.** The Red and Bluetooth pages were one
  `Grouped` surface; the cards are the surfaces now and the page is a plain
  item, so the window background shows between cards.

## Result

| Command | Where | Exit |
| --- | --- | --- |
| `cargo test -p cuprita-core` (68 passed: 40 unit, 28 integration) | `celestina-rs/` | 0 |
| `cargo clippy -p cuprita-core --all-targets -- -D warnings` | `celestina-rs/` | 0 |
| `cargo run -p cuprita-core --example snapshot` (live: the connected Wi-Fi and the connected wire each report an IPv4 address; the others `None`) | `celestina-rs/` | 0 |
| `cargo fmt --all --check` | `cuprita/` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | `cuprita/` | 0 |
| `cargo test --locked` (9 passed) | `cuprita/` | 0 |
| `cargo build --release --locked` | `cuprita/` | 0 |
| `sh cuprita/scripts/qml-tests.sh` (44 passed) | root | 0 |
| `sh cuprita/scripts/smoke.sh --binary <shared target>/release/cuprita` (`networks=4 devices=2 endpoints=3 streams=2 sink=40 source=50`) | root | 0 |
| `bash scripts/qmllint-cxxqt.sh cuprita` (0 warnings) | root | 0 |
| `bash scripts/check-architecture-contract.sh` (radius and glass-canvas OK) | root | 0 |
| `python3 scripts/check-language-contract.py` | root | 0 |
| `bash scripts/check-documentation-contract.sh` | root | 0 |

New tests: `the_first_ipv4_entry_is_the_address` (unit),
`disconnecting_the_wired_link_drops_its_address` and the address assertions
in `the_scripted_network_starts_wired_and_ordered` (integration);
`test_the_connection_card_shows_the_wired_link_and_its_address`,
`test_the_networks_card_leaves_out_the_connection_and_the_vpn`,
`test_without_a_connection_the_card_says_so`,
`test_the_vpn_card_switches_the_vpn` (QML). The network tests that counted
four list rows now count the two of the networks card.

The three pages were also rendered offscreen at 640 × 560 over the stand-ins
(a throw-away `grabImage` test, not committed) and looked at: the labels,
16 px insets, hairlines, bars and card gaps read as designed.

## Addition: the loading state

At the author's request after approving the layout, a second commit adds a
visible working state.

- `NetworkController`, `BluetoothController` and `AudioController` expose
  `loaded` (false until their first snapshot is applied, true from then on)
  beside `busy`.
- While not loaded, the networks, devices, applications, output and input
  cards show a centred loading row (`LoadingLine.qml`: the refresh spinner and
  a muted line) instead of their rows or the empty line; the empty lines and
  the no-connection line appear only once loaded.
- While loaded and busy, `SectionCard.working` shows a small spinner beside
  the card's label (as tall as the label, so the card never moves); the rows
  stay.
- The spinner is one component, `Spinner.qml`, also used by the Bluetooth
  search; its turn stops under `reducedMotion`.
- The stand-ins carry `loaded` (true, like the fakes) and `snapshot()`; one
  QML test per page holds `loaded` false, sees the loading row, delivers the
  snapshot and sees it go.

| Command | Where | Exit |
| --- | --- | --- |
| `cargo test -p cuprita-core` (68 passed) | `celestina-rs/` | 0 |
| `cargo fmt --all --check` | `cuprita/` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | `cuprita/` | 0 |
| `cargo test --locked` (9 passed) | `cuprita/` | 0 |
| `cargo build --release --locked` | `cuprita/` | 0 |
| `sh cuprita/scripts/qml-tests.sh` (48 passed) | root | 0 |
| `sh cuprita/scripts/smoke.sh --binary <shared target>/release/cuprita` (`networks=4 devices=2 endpoints=3 streams=2 sink=40 source=50`) | root | 0 |
| `bash scripts/qmllint-cxxqt.sh cuprita` (0 warnings) | root | 0 |
| `bash scripts/check-architecture-contract.sh` | root | 0 |
| `python3 scripts/check-language-contract.py` | root | 0 |
| `bash scripts/check-documentation-contract.sh` | root | 0 |

## Review fixes

From the coordinator's review (forget stays a trash button, as the author
accepted):

- A VPN's switch reads «Activar …» again: `SettingRow.accessibleName`
  (default the label), set by the VPN rows; asserted in
  `test_the_vpn_card_switches_the_vpn`.
- `SectionCard.rowInset` is the one definition of the row inset; rows,
  `EmptyLine` and `RowDivider` take it as a required `inset` from their card.
- `SignalBars`: gap `spaceXs` (the smallest spacing token) and named
  `count`, `gap` and `heightRatio`.
- The profile card is `width: parent.width` like its siblings.
- `RowFilter.refilter()` coalesces with `Qt.callLater`, with a comment on
  its cost, and exposes `pending` so a card never reads empty before the
  pass runs.

| Command | Where | Exit |
| --- | --- | --- |
| `sh cuprita/scripts/qml-tests.sh` (48 passed, 7.8 s) | root | 0 |
| `bash scripts/qmllint-cxxqt.sh cuprita` (0 warnings) | root | 0 |
| `python3 scripts/check-language-contract.py` | root | 0 |
| `bash scripts/check-documentation-contract.sh` | root | 0 |
| `cargo build --release --locked` | `cuprita/` | 0 |

Three tests then sat out `waitForRendering`'s 5 s timeout: each called it
on a scene with nothing left to draw (after a `tryCompare` whose polling had
already let the frame render, or a second wait right after a first), so no
new frame came. They now check the clicked switch is laid out, let the
no-flash test run a few turns with `wait`, and keep one frame wait in the
audio test; the suite is back to about 8 s.

## Limits

- No empty-line flash between `loaded` and the rows. In Rust the order
  holds because each model's queued delivery of the first snapshot is posted
  (during `publish`) before the controller posts its `loaded` flip, on the
  same GUI thread, so the models apply it first; a model subscribing after
  the first snapshot would get `latest` after `loaded` (all models exist at
  startup today; `HubState::subscribe` says so). In QML, `RowFilter`
  coalesces its passes with `Qt.callLater` and reports `pending` until the
  pass runs; the empty lines wait for it. The test
  `test_the_empty_line_never_shows_between_loaded_and_the_rows` pins the QML
  contract: the stand-in delivers the rows, then flips `loaded`, and the
  empty line never shows.
- The address comes from `Ip4Config`, read when the network snapshot is
  taken; a DHCP renewal that changes it without any other NetworkManager
  change shows on the next snapshot.
- Only IPv4 is shown; an IPv6-only link shows its kind alone.
- The author checked the layout live on 2026-10-08.

## Follow-up

Author checks pending: `VAL-C`, `VAL-D`, `VAL-E` in
[`../../VALIDATION.md`](../../VALIDATION.md), now on the new layout.

## Landing

- **Base revision:** `122499cb668f1186f5ee5fa896ceb223c235191f`
- **Check:** `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1d27c1c699f86a59e3588bcdabd6db72ace8f76b7a27215efcb9581166e0cd19, verification_fingerprint sha256:f9e9f4337bac15c2a74d577537e62f02aef00668192d1505933d5c7ec92ea550; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7bfc17e11e8b3e0fe86fd06841b8f90063a27dfba8e75d1c0a318d25ba9a6e17, verification_fingerprint sha256:4debca234a4e56fc74926b00dc4855e4d7fe325b9a9d125d56de6c084f44b342
- **Deploy:** after the push: cuprita: deploy-production.sh, status-production.sh
