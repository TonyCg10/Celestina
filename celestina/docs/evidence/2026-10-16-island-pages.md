# Evidence: CEL-1-D — the Island pages, page tabs and the overflow menu

- **Date:** 2026-10-16
- **Scope:** unit CEL-1-D on `unit/celestina/CEL-1-D`, stacked on CEL-1-C:
  `celestina/src/island/` (state, view, the `pages/` Activity, Notifications and
  Tray, the page tabs, the overflow `menu`), `celestina/src/{app,copy,main}.rs`,
  `celestina/src/providers/niri.rs` (window icons), and
  `celestina/crates/celestina-wayland/src/{popup,runtime,handlers}.rs`
- **Environment:** Linux 7.2.10-1-cachyos; the toolchain
  `celestina/rust-toolchain.toml` pins; every cargo command below ran from
  `celestina/`. The unit's last commit at the time of the run is 5d04d853.
- **Artifact:** the release binary built in this session worktree at 5d04d853;
  it identifies this run only and is not reproducible across landings (the
  landing builds and seals its own)

## Procedure

From `celestina/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
sh scripts/smoke.sh --binary <target>/release/celestina
```

From the worktree root:

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
```

## Result

- **Exit:** 0 for every command above.
- **Tests:** `cargo test --workspace --locked` passes at 5d04d853: the
  `celestina` library has 109 tests and `celestina-wayland` 41. After the
  final-review fix wave (the single growth clock, the Notifications switch
  state, the keyboard-opened menu's focus) it passes with 113 and 41; that
  wave re-ran fmt, clippy, the tests and the language and documentation
  guards, and the release build and the smoke were repeated at 9173b4a9
  (below). The new ones
  cover the page switch and its slide (direction, same page, reduced motion,
  a switch during a slide), the page size and its clamp to the surface, the
  size animation, the three pages and their tabs, Activity's rows, icons,
  title cut, row cap and focusable set, the owner lines, the menu's size,
  placement, keys, choices and disabled row, the Island's handling of the
  menu's close reasons, the reopening, the stale-choice slot, the grab
  serial (`menu_grabs_with_the_last_input_serial_pointer_or_key`), a page
  request during the growth riding the growth's clock (on open and on
  reopen), the Notifications switch drawn with the setting in force, and the
  keyboard-opened menu focusing its first enabled row.
- **Smoke:** the release build ran under the nested niri and exited 0 with
  `smoke: winit frames=3 rest_frames=0 n=1 late=0 comp_late=0 cpu_p50_ms=5.986`,
  `rss_mib=72.4` and an atlas occupancy of 0.0022, then `OK`; repeated at
  9173b4a9 with `cpu_p50_ms=1.520`, `rss_mib=72.6`, occupancy 0.0021, `OK`.
- **Guards:** `check-architecture-contract.sh`, `check-language-contract.py`,
  `check-documentation-contract.sh` and `version_tool.py check` all exit 0.
- **Canonical owners:** the pages and the menu in `src/island/`; the product
  copy in `src/copy.rs`; the grab serial rule in `celestina-wayland`'s
  `popup.rs`; the do-not-disturb flag through the existing `ShellSettings`.

## Limits

Nothing was checked in a real session. The smoke starts the shell and counts
frames; it opens no page and no menu. Not exercised: the popup grab and its
dismissal by Escape and by a click outside, the menu's placement under the
button, the compositor's blur behind the menu, the slide between pages, the
page resize, the keyboard focus ring and traversal, Activity over many real
windows and icons, and any behaviour with another program holding the
notifications or tray names. These are the author's rows VAL-CEL-ISLAND,
VAL-CEL-POPUP and VAL-CEL-KEYBOARD in [VALIDATION.md](../../VALIDATION.md).
`build-production.sh`, `verify-production.sh` and the deploy were not run:
the landing seals and deploys.

## Rulings and known limitations

- The open tab is drawn as an Accent pill with AccentInk text; the plan names
  ControlFill pills and leaves the selected look unspecified. A one-line token
  swap if the author prefers ControlFill.
- Reopen: the menu only opens while the page is open, so the reopen row
  makes the page snap to the capsule and grow again from fresh views. The plan
  does not define it; a views refresh is the cheap alternative.
- The do-not-disturb write is synchronous on the loop thread: one small atomic
  file write through the existing `ShellSettings` API. Nothing honours the
  flag until CEL-1-F's server reads it; moving the write to a worker belongs
  with that server. Cost: a possible few-millisecond stall on a disk hiccup.
- Owner detection is not wired: `App::set_owners` has no production caller
  until CEL-1-F and CEL-1-G fill it, so both explanation pages show their
  empty state and the do-not-disturb row is enabled.
- The view tree has no disabled state for assistive technology: disabled is
  expressed as not interactive, not focusable and faint roles. The tabs and
  the menu expose no role or selected state either.
- The slide animates only the incoming page; the outgoing page's nodes vanish
  at once.
- Activity lists at most 24 rows and builds only the rows that fit the
  surface; the rest are neither drawn nor focusable. With less room than one
  row it shows the empty-state copy.
- The menu opened by keyboard starts on its first enabled row (by the
  pointer, with none); it has a fixed width of 280, and the More button gets
  no active fill while the menu is open.
- A page's size request while the growth is in flight becomes the growth's
  target instead of starting a resize clock: one clock per gesture. Only a
  request on a settled open page animates the size.
- The Notifications page's do-not-disturb switch shows the setting in force
  and stays disabled (the overflow menu changes it) until CEL-1-F's server
  exists, and while another program holds the name.
- `celestina-wayland` gained `SurfaceCtl::stop` (the menu's Quit reaches the
  app through `Shell::input`, which has only a `SurfaceCtl`) and `GrabSerial`
  in `popup.rs`, which replaces the runtime's last-input serial and picks the
  newer of the pointer and key presses by arrival order; `handlers.rs` records
  into it. The app's painter is now a shared `Rc<RefCell<Painter>>` so the menu
  draws with the same atlas.

## Follow-up

The author's rows VAL-CEL-ISLAND, VAL-CEL-POPUP and VAL-CEL-KEYBOARD in
[VALIDATION.md](../../VALIDATION.md); the tray's nested menus in CEL-1-G; the
notifications and tray owner detection in CEL-1-F and CEL-1-G.

## Landing

- **Base revision:** `08949c341bb87fd125c35ca792fc369a7b9654e0`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:98f94a6c76bc6c567ab14d44657cb3cd9e9535bad11f848a604008fb65d73404, verification_fingerprint sha256:a6f31bd3505f0b115dbe0c15b9bf85407164bd6414b6060c284263100bd27472
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
