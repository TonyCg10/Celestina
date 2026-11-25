# Evidence: the session menu (CEL-I-006)

- **Date:** 2026-11-25
- **Scope:** unit CEL-I-006 on branch `unit/celestina/CEL-I-006`, opened from
  `unit/celestina-style/STYLE-ICONS-1` (which adds the `lock`, `moon` and
  `log-out` glyphs). It adds the `session` piece (a `power` glyph, last in the
  bar's end zone) and its menu: the lock row and the suspend row act at once; the
  log-out, reboot and power-off rows turn into an inline danger card with a
  cancel button and the action. The toolkit gained the four danger roles
  (`DangerInk`, `DangerFill`, `DangerBorder`, `DangerFillInk`), a one-px box
  stroke, and a rule that a coloured interactive box keeps its fill under the
  pointer.
- **Preview approval:** the author approved variant A of the session-menu
  board on 2026-11-25. The board is a design canvas private to the author; the
  values in the plan are the ones it used.
- **Environment:** CachyOS, kernel 7.3.5-2-cachyos, the toolchain the
  repository pins, plus `cargo +1.100` for the second clippy pass.
- **Artifact:** debug `celestina` from the shared worktree target. Not the
  production artifact: the landing builds that.

## Design decisions the plan left open

- The confirmation card's gap between the question and the buttons is
  `spacing.sm`.
- Keyboard focus goes to the cancel button whenever a confirmation card appears,
  whether the row was chosen with the pointer or the keyboard. Going back with
  Escape or Enter on the cancel button returns the focus to the row left; a pointer
  click on the cancel button leaves the focus alone.
- While a card is open the other rows stay live: choosing the lock row or
  the suspend row closes at once; choosing another dangerous row moves the card to
  that row.
- The popup placement is the level popups' own
  (`osd::popup::placement`: bottom edge, bottom gravity, offset `spacing.sm`),
  reused rather than copied. Its size is asked again through
  `SurfaceCtl::resize_popup` when a card comes or goes and when the text scale
  changes the wrapped question.
- A box with no fill or the control fill still swaps to `ControlHover` and
  `ControlPressed` under the pointer (so every transparent piece keeps its
  plate). Any other fill (the accent, the danger tone) is kept, with the same
  plate drawn on top as a wash. This also changes the polkit prompt's
  authenticate button, which lost its accent under the pointer before.
- The menu opens without a keyboard flag, so a menu opened by the keyboard
  starts with nothing focused; the first Down focuses the lock row.

## Procedure

From `celestina/`:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

and from the worktree root `bash scripts/check-architecture-contract.sh`,
`bash scripts/check-documentation-contract.sh` and
`python3 scripts/check-language-contract.py`. The smoke bar phase ran under
`env -i` with only `HOME`, `PATH`, `XDG_RUNTIME_DIR`, `USER`,
`WAYLAND_DISPLAY`, `LANG`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0` and
`CARGO_TARGET_DIR`:

```sh
timeout -k 60 2700 sh scripts/smoke.sh --binary <target>/debug/celestina --only bar
```

## Result

- **Nested look:** [bar-end](2026-11-25-cel-i-006-session-menu/bar-end.png) — the end of the bar: volume, clipboard and the new power piece (full frame: [bar-end-full](2026-11-25-cel-i-006-session-menu/bar-end-full.png)); nested niri, fake audio, not the live session
- **Nested look:** [menu](2026-11-25-cel-i-006-session-menu/menu.png) — the pointer click on the piece opens the menu: lock, suspend, a divider, log out, restart, power off (full frame: [menu-full](2026-11-25-cel-i-006-session-menu/menu-full.png))
- **Nested look:** [confirm](2026-11-25-cel-i-006-session-menu/confirm.png) — the click on the power-off row opens the confirm card with the cancel button focused; the confirm button was not clicked (full frame: [confirm-full](2026-11-25-cel-i-006-session-menu/confirm-full.png))
- Format, both clippy passes: clean.
- `cargo test --workspace --locked` in `celestina/`: 46 suites, 2392 passed,
  0 failed, 2 ignored (summed over every `test result` line). A first run
  failed `connectivity::tests::failed_retries_start_no_watcher_and_a_lane_keeps_one_for_life`
  once under load; it passes alone and in the second full run, and the unit
  does not touch it.
- `cargo test --workspace --locked` in `celestina-rs/`: 2287 passed, 0
  failed.
- New tests: `a_box_stroke_is_kept_and_other_kinds_ignore_it`,
  `a_coloured_fill_keeps_its_colour_under_the_pointer_with_a_wash_on_top`,
  `the_outline_lies_inside_the_box`, the theme role mapping (20 roles), the
  copy pin `the_session_menu_copy_is_pinned`, nine `session_menu` tests, the
  piece test, the island tests for the anchor and the popup bookkeeping, and
  the settings tests with `"session"` last in the default end zone.
- Smoke `--only bar`: OK (`late=0`, `comp_late=0`, `rest_frames=0`).
- Guards: see the landing; they ran green on the commits.
- After the review fixes: `cargo test --workspace --locked` in `celestina/`
  2393 passed, 0 failed; both clippy passes clean.
- **Review:** `/simplify` in the session (the tray menu's arrow-key mapping
  shared instead of copied), then a final review on Opus 5.5 with high
  reasoning, round 1 of 2. Fixed: the confirm card's buttons could sit under
  the last pixels of the row it replaced, so a quick double-click on Restart
  or Power off could confirm it (the question now keeps a minimum height,
  test `a_double_click_on_a_dangerous_row_never_reaches_its_action`); a
  chosen action was kept when its close was dropped and could fire on a later
  popup's close (it is now taken as the close arrives, and dropped when an
  output takes the menu's Island with it). Round 2 on Opus 5.5 read the fixes:
  no defect left. Deferred: below.

## Limits

- The nested screenshots above match variant A of the approved preview in
  layout, rows, icons and colours; they are a nested niri with fake audio,
  not the author's session.
- The fix that drops a pending choice when its close is dropped (`app.rs`)
  has no unit test: the app's input path needs a live runtime.
- No verb was ever run for real: the lock, the suspend, the log-out, the
  reboot and the power-off were never sent. The tests prove which verb each
  row maps to and that the app posts it after `PopupClosed`, nothing more.
- The compositor side of `xdg_popup.reposition` (the menu growing with its top
  edge fixed) is the level popups' tested path, not exercised here again.
- The author's live row is `VAL-CEL-I-006`.

## The author's layout

`~/.config/celestina/shell.json` sets `bar.end` explicitly, so the new default
does not reach it. The line to change is:

```json
"end": ["bubbles","sysmon","tray","network","bluetooth","volume","brightness","clipboard","media","battery","power-profile","session"]
```

## Follow-up

- niri shows its own confirmation after the log-out row (`Quit` with
  `skip_confirmation: false`). The menu asks first, so the author confirms
  twice. Left as is; a later unit may send the quit without niri's dialog.
- A keyboard-opened menu could start with the first row focused, as the
  overflow menu does; the island action carries no keyboard flag yet.
- The hover rule now keeps any non-control fill and washes it: the active
  Island tab and the polkit authenticate button keep their accent under the
  pointer, and the overview's selected window tile (already `ControlHover`)
  is washed twice when hovered.

## Landing

- **Base revision:** `192b8b4f32cd8a11c69b915aea42d0822dfe923e`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:f83e02ad7ff8253571a3de98ba3ea05f5dd4105c45ee6be08ae8ee7ceeb3d59a, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:94ff7e9197411907a54762ac8aa6b60977d041cfab54317e749c1feae95a68aa, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
