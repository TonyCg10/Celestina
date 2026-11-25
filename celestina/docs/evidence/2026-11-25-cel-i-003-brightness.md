# Evidence: the brightness of every monitor (CEL-I-003)

- **Date:** 2026-11-25
- **Scope:** unit CEL-I-003 on branch `unit/celestina/CEL-I-003`, opened from
  `main` at `6bbefc1d`. With two monitors or more the brightness popup shows
  the All heading with the mean level and a slider that sets every monitor
  with a reading, a divider, and one compact row per monitor (name, slider,
  percent). With one monitor it is the single slider of old. The DDC worker
  now serves pending writes round-robin, so a drag over every monitor does not
  starve the later connectors.
- **Preview approval:** the author approved variant C on 2026-11-25. The board
  is a design canvas private to the author; the plan carries the values it
  used (name column 96 px, percent column 38 px, card 320 px wide).
- **Environment:** CachyOS, kernel 7.3.5-2-cachyos, the toolchain the
  repository pins, plus `cargo +1.100` for the second clippy pass.
- **Artifact:** debug `celestina` from the shared worktree target. Not the
  production artifact: the landing builds that.

## Design decisions the plan left open

- The All heading reads "Todos" (`copy::BRIGHTNESS_ALL`).
- The popup lists monitors only when `Readings::monitors_for` returns two or
  more; one listed monitor keeps the single slider, whose request is
  `Control::Brightness` as before.
- The All slider shows the mean of what the popup shows, so it follows the
  pointer while every row takes the new level. Its value is disabled when no
  monitor has a reading.
- A row with no reading is greyed (name in `TextFaint`), has no percent text
  and a disabled slider. The percent cell stays, so the columns do not move.
- `LevelCore::cut_monitors(theme, now, fit)` takes a measure that also gets
  the row's weight. `LevelPopup::frame` calls it before `sync` and passes the
  frame's own painter measure (the painter is borrowed there). It cuts on the
  first frame and when the theme changes, and rebuilds the card.
- `MonitorMirror` has `row_count()`, not `len()` (clippy's `len_without_is_empty`).
- `set_model` trims, bounds to `MAX_TEXT_UNITS` and trims again; `Unknown`
  and an empty name remove the entry.
- A second internal output would map to the same backlight target: the list
  skips any target already listed.
- `Controls::refresh` compares the listed monitors' levels only when the popup
  lists several.

## Fixes of review round 1

- **Quiet after closing.** A popup that wrote several monitors left the
  display quiet for one `BRIGHTNESS_SETTLE` (6 s), but the DDC worker writes
  and reads the monitors one at a time, at one to two seconds each, so the
  last read-back could come after it and show the OSD. The popup now counts
  the distinct rows it asked a level of (`Controls` notes each
  `Set(Monitor(_))`), `Suppression::Closed` carries that count as `written`,
  and the display releases at `now + BRIGHTNESS_SETTLE * (written - 1)`, so the
  quiet lasts one settle for each monitor written (one for a single
  monitor, as before). It is the simplest cover: no change to
  `celestina-rs`, since `release` only uses `now` to place the bound. Tests:
  `the_quiet_after_closing_lasts_a_settle_for_each_monitor_written` and the
  `written: 3` of `a_multi_monitor_popup_quiets_brightness_for_the_whole_settle`
  (red with `written` fixed at 0).
- **Adoption by row.** `MonitorMirror::adopt` replaced every row at once when
  any reading changed, so releasing All sent rows back to old readings. Each
  row is now adopted only when its own reading differs from the last taken in
  for it. Test: `a_late_reading_of_one_row_does_not_move_another` (red before).

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
`python3 scripts/check-language-contract.py`. The smoke ran under `env -i`
with only `HOME`, `PATH`, `XDG_RUNTIME_DIR`, `USER`, `WAYLAND_DISPLAY`,
`LANG`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0` and `CARGO_TARGET_DIR`:

```sh
timeout -k 60 2700 sh scripts/smoke.sh --binary <target>/debug/celestina --only osd
```

## Result

- Format, both clippy passes: clean.
- `cargo test --workspace --locked` in `celestina/`: 46 suites, 2414 passed,
  0 failed, 2 ignored (summed over every `test result` line).
- New tests: in `osd::readings`
  `monitors_list_the_bars_output_first_then_the_others_by_connector`,
  `a_monitor_is_named_by_its_model_or_else_its_connector`,
  `only_reported_monitors_are_listed_besides_the_bars_own`,
  `the_backlight_is_listed_as_the_internal_panel` and
  `the_list_stops_at_max_monitors`; in `osd::monitor_rows`
  `mean_level_rounds_half_up_and_skips_unknown`; in `osd::popup`
  `one_monitor_keeps_the_single_slider_popup`,
  `several_monitors_open_the_list_at_the_audio_width`,
  `a_todos_drag_sets_every_known_monitor_and_sends_one_request_each`,
  `a_monitor_without_reading_is_greyed_and_takes_no_input`,
  `todos_then_a_row_sends_each_monitors_last_value`,
  `readings_update_rows_in_place_and_the_size_stays` and
  `the_names_are_cut_at_their_rows_weight_once_per_theme`, and the extended
  `requests_become_the_workers_commands`; in `osd::control`
  `a_multi_monitor_popup_quiets_brightness_for_the_whole_settle` and
  `refresh_brings_every_listed_monitor_up_to_date`; in
  `providers::brightness` `a_drag_over_every_monitor_writes_each_in_turn`
  (red with `pop_first`: writes `["1","1","2"]`; green with the round-robin:
  `["1","2","1"]`).
- Smoke, phase `osd` (`--only osd`): OK. The phase checks the display and the
  verbs, not the popup: a nest runs with `CELESTINA_DDC=0` and
  `CELESTINA_BACKLIGHT=0`, so the brightness piece and its popup do not
  appear there. Tests fake every monitor. The live look and the DDC behaviour
  wait for VAL-CEL-I-003.

## Limits

- The shell was never run live. The look of the list (columns, bold primary
  name, grey row), the order of the writes on real DDC, and the model that
  niri's `wl_output` reports for the author's monitors are not observed.
- `RuntimeCtl::output_model` has no unit test: it needs a live sctk state.
- A monitor added after the popup opens appears the next time it opens.

## Follow-up

- Not fixed, as it was there before this unit: `stop()` and `REAP_GRACE` of
  the DDC worker.
- Not fixed: the `u8::MAX` fallback when a row index does not fit a `u8`
  (the list holds at most `MAX_MONITORS`, so it cannot happen).

## Landing

- **Base revision:** `6bbefc1db9fe98e2ed10da4731fe899a92ee3d87`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4cf4e0f5c13f73c59113aa1d7bfa87df9656bd4e5225811bbe647084cb52999b, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
