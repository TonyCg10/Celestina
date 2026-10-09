# The Appearance section — CUP-1-I

- **Date:** 2026-10-09
- **Scope:** `CUP-1-I` of
  [`../plans/active/2026-10-08-cup-1-foundation.md`](../plans/active/2026-10-08-cup-1-foundation.md),
  which is `CONV-1-C` of the suite program
  ([`../../../docs/plans/active/2026-10-09-suite-conventions.md`](../../../docs/plans/active/2026-10-09-suite-conventions.md)):
  `cuprita/src/controller/` (new `appearance.rs`; `app.rs`, `mod.rs`),
  `cuprita/src/backend.rs`, `cuprita/build.rs`, `cuprita/qml/Main.qml`,
  `cuprita/qml/pages/AppearancePage.qml` (new),
  `cuprita/qml/components/` (new `ScaleChoice`; changed `Sections`,
  `SectionShortcuts`, `SettingRow`), `cuprita/tests/qml/`
  (new `tst_appearance_page.qml` and `fakes/FakeAppearanceController.qml`;
  `tst_strip.qml`), `cuprita/scripts/smoke.sh`, `cuprita/README.md`,
  `cuprita/STATUS.md`, `cuprita/VALIDATION.md`, the ledger
- **Environment:** Qt 6.12, cxx-qt 0.9.1, `qmltestrunner` offscreen
- **Artifact:** the release binary in the shared Cargo target, not installed
- **Change kind:** maintenance (no version change; no history row)

## Procedure

Spec §6 of
[`docs/superpowers/specs/2026-10-09-suite-conventions-design.md`](../../../docs/superpowers/specs/2026-10-09-suite-conventions-design.md)
makes Cuprita the one place where the suite's shared appearance is edited.

| # | Requirement | What this unit does |
| --- | --- | --- |
| 1 | A fourth section in the strip, Ctrl+4 | `Sections.all` gains `appearance` with the `paintbrush` glyph (the closest `CelestinaIcons` has: no contrast, palette or text-size glyph exists); `SectionShortcuts` adds Ctrl+4; `Main.qml` stacks `AppearancePage` fourth |
| 2 | One card in the CUP-1-H grammar | `AppearancePage`: one `SectionCard` with a `SettingRow` for reduced motion, a `RowDivider`, and the text-size row; a `LoadingLine` until the first reading |
| 3 | Forced by the environment | `forcedByEnvironment` is `env_forces_reduced_motion()`; the switch then shows on, is disabled, and the row's new `hint` line says so; `setReducedMotion` is ignored while forced |
| 4 | Four sizes, keyboard-walkable | `ScaleChoice` (see decisions): `"Compacto"`, `"Normal"`, `"Grande"`, `"Muy grande"`; one Tab stop, Left/Right walk the sizes (wrapping), Home/End reach the ends, each step saved at once |
| 5 | Saves off the Qt thread; one notice on failure | `AppearanceController` drives a `Worker` over `backend::AppearanceStore`: a change reads the file, sets the one value and calls `celestina_settings::save`, all on the worker; a failure is one error notice in the window's pill (Spanish; the detail is logged where the job runs, not by `message_es`) |
| 6 | Cuprita previews itself | The window keeps following the file through `CupritaController` (CONV-1-B); the section's own values come from a re-read after each save and from `celestina_settings::watch` for changes made elsewhere |
| 7 | Fakes never touch `~/.config` | Under `CUPRITA_FAKE=1` the store is `FakeAppearance`, kept in memory; its writes go straight to the window through `app::preview`, so the fake window previews too. The smoke also runs under a scratch `XDG_CONFIG_HOME` |
| 8 | Accessible names | The switch is a check box named `"Reducir el movimiento"`; the choice is a tab list named `"Tamaño del texto"`, each size a tab named by its word, checked and selected when current |

### Design decisions

- **A local `ScaleChoice`, not `CelestinaSegmentedControl`.** The shared
  control was registered first; its delegate reaches outer ids without
  `pragma ComponentBehavior: Bound`, which grew Cuprita's qmllint count from
  9 to 14 (five `unqualified` warnings), and `celestina-style/` is outside a
  `cuprita:` commit's scope. `ScaleChoice` draws the same plate and
  selected segment, words only, with `Bound`, the same stateless
  `activated(index)` contract and the same focus model (the current segment
  holds the focus, its ring on `visualFocus`); unlike it, it shrinks to the
  row and elides its words when the card is narrower than they are. Adding the pragma to the shared control would
  let a later unit replace it.
- **The size choice below its label.** Four worded segments beside the
  label do not fit the 560 px minimum window; the label sits on its own
  line and the choice under it.
- **A change re-reads before it saves.** Each command reads the current file
  on the worker and changes only its own value, so two quick changes, or a
  change made in another window, are never overwritten by a stale copy.
- **Smoke.** The smoke report adds `appearance=<textScale>` once the
  section's first reading arrived, and expects `appearance=normal` from the
  fake store.

## Result

| Command | Where | Exit |
| --- | --- | --- |
| `cargo fmt --all --check` | `cuprita/` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | `cuprita/` | 0 |
| `cargo test` (11 passed) | `cuprita/` | 0 |
| `cargo build --release --locked` | `cuprita/` | 0 |
| `sh cuprita/scripts/qml-tests.sh` (66 passed) | root | 0 |
| `sh cuprita/scripts/smoke.sh --binary <shared target>/release/cuprita` (`networks=4 devices=2 endpoints=3 streams=2 sink=40 source=50 appearance=normal`) | root | 0 |
| `bash scripts/qmllint-cxxqt.sh cuprita` (9 baseline warnings, unchanged) | root | 0 |
| `bash scripts/check-architecture-contract.sh` | root | 0 |
| `python3 scripts/check-language-contract.py` | root | 0 |
| `bash scripts/check-documentation-contract.sh` | root | 0 |

New tests: `a_change_keeps_the_other_value` (Rust);
`test_the_switch_and_four_sizes`, `test_choosing_large_sets_large`,
`test_the_arrows_walk_the_sizes`, `test_space_toggles_reduced_motion`,
`test_the_environment_forces_reduced_motion`,
`test_loading_until_the_first_reading`, `test_home_end_and_the_wrap`,
`test_the_current_size_holds_the_focus`,
`test_a_failed_save_keeps_the_switch_and_says_so`,
`test_the_choice_stays_inside_a_narrow_card` (QML); the strip tests count four
sections.

## Limits

- While `CELESTINA_REDUCED_MOTION` is set, `load()` reports reduced motion
  on whatever the file says, so changing the text size then also writes
  `reduced_motion = true` to the file. `celestina_settings` offers no read
  of the file without the override; a later suite unit can add one.
- Live following across windows (a change here redrawing every other open
  suite window) is proven only by `VAL-F`: the automated tests run the page
  on a stand-in and the smoke on the in-memory fake store.
- Walking the choice logs Qt's "Cannot set activeFocusOnTab to false once
  item is the active focus item" in the tests: the focused segment gives up
  its Tab focus as the current one moves, exactly as in
  `CelestinaSegmentedControl`, whose focus model this follows.
- The window's switch is not moved locally: it shows the controller's value,
  which changes once the worker has saved and read back.

## Follow-up

Author check pending: `VAL-F` in [`../../VALIDATION.md`](../../VALIDATION.md).

## Landing

- **Base revision:** `1225374bd128c70e3aa54320466b7b4c9914c1af`
- **Check:** `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1e186310a7a47c204037079d5b7a67dd7d32988be73501b5291b99f605cb8906, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606
- **Deploy:** after the push: cuprita: deploy-production.sh, status-production.sh
