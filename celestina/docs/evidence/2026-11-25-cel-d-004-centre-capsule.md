# Evidence: one centre capsule with time, date and weather (CEL-D-004, Part C)

- **Date:** 2026-11-25
- **Scope:** `CEL-D-004`, Part C of the capsule sheets plan: the Island capsule in the centre of the bar shows the time, the date and the weather; a click on it opens the clock page; the clock and weather pieces leave the default centre. Task C1 is committed (`ce3f4972`, merged with `main` at `f419a2f8`); this file is task C2, re-checked after the weather fix `9d8b9097`
- **Environment:** worktree `celestina-CEL-D-004` at `9d8b9097` (the shots 01 and 02 were taken at `f419a2f8` with a `weather` piece placed; shot 03 and the smoke line were re-taken at `9d8b9097`); debug build in a private target directory; nested niri at 2560x1440 logical, scale 1.5 (3840x2160 pixels), output `winit`; `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, `--own-bus`, no system bus; fake `curl` first on `PATH` answering the forecast body `{"current":{"temperature_2m":12.4,"weather_code":3,"is_day":1}}`; the location Dublin (53.35, -6.26) in the kit's `shell.json`. The nest has no network
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Design and plan:** [design](../../../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md), [plan](../../../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md)
- **Author's line to change:** in `~/.config/celestina/shell.json`, `"centre"` is `["weather","clock"]` today; change it to

  ```json
  "centre": []
  ```

  The default layout of the code already has an empty centre (`BarLayout::default().centre`), so the author's file is the only change.

## Procedure

```sh
CARGO_TARGET_DIR=<private tgt> cargo build --locked --bin celestina
CARGO_TARGET_DIR=<private tgt> cargo build --locked -p celestina-lock -p celestina-lock-verify
CARGO_TARGET_DIR=<private tgt> cargo build --locked --example fake-mpris-player
CARGO_TARGET_DIR=<private tgt> cargo build --locked --offline --example fake_system_bus
timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina --only bar,glance
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

The nested shots came from the capture kit (host-wide nest lock, `env -i`, fakes first on `PATH`, `dev-session.sh --own-bus --once --probe`), the binary pointed at the private build, pointer input from a virtual pointer, and `niri msg action screenshot-screen` for each frame, taken 2 s after the action. Probes talk to the shell only through the guarded `cel` function.

## Result

- **Smoke (at `9d8b9097`):** `smoke: OK (only: glance,bar)`, exit 0 (the phases ran in the order glance,bar). Lines: `bar on: winit frames=70 rest_frames=0`; `glance-weather: frames=143 rest_frames=1 curl_calls=1`, `glance-weather: 1 request(s), forecast URL, --disable, no label`
- **Checks:** see the "Checks" list below for what ran in this task
- **The capsule with a weather reading** ([crop](2026-11-25-cel-d-004-centre-capsule/01-rest-capsule-with-weather.jpg)): at rest the capsule reads the time `22:41`, the short date (`copy::short_date`), a cloud icon and the temperature `12°`. The time is the heavier, brighter run; the date and the weather are the lower tone
- **The clock page after a click** ([crop](2026-11-25-cel-d-004-centre-capsule/02-clock-page-after-click.jpg)): one click on the capsule at (1280, 28) opened the clock sheet (its title and buttons are product copy in `src/copy.rs`) with the seconds, the month with its navigation, and at its foot a weather line with the temperature, the condition and the place with the update time. The Island's tab row shows the calendar tab lit
- **The author's exact config, after the weather fix** ([crop](2026-11-25-cel-d-004-centre-capsule/03-rest-capsule-author-config.jpg)): with `"centre": []`, no `weather` or `clock` piece placed anywhere and the Dublin location set, the capsule reads the time `22:48`, the short date, a cloud icon and `12°`. The probe waited 15 s after the shell was up; the fake `curl` logged the one forecast request. The crop is 600x90 pixels of the 3840x2160 frame
- **History:** the first C2 run (before `9d8b9097`) showed the time and the date only with this config, because `Wants::of` in `glance.rs` started the weather worker only for a `weather` or `clock` piece. The fix reads the weather whenever a location is set; the re-check above is the proof

## Decision record

The weather worker now runs whenever `shell.json` holds a location, with or without a `weather` or `clock` piece (capsule sheets design §4.5, approved by the author). ADR 0016 Decision 3 is amended with a dated section (2026-11-25, CEL-D-004); the `sysmon` rule is unchanged and with no location there is still no network. To stop the weather request, remove the location.

## Limits

- The weather was a faked reading (fake `curl`, fixed body); the nest has no network, so the real forecast request and its timing were not seen
- The workspace dots of the capsule (`capsule_shows_workspaces`) were not checked in these shots
- No frame in flight: the shots are settled frames, taken 2 s after the click; the rebuild on a change of `today` or `weather` (one frame) is covered by unit tests in `src/island/mod.rs` (`a_new_day_or_reading_rebuilds_the_capsule_once` and `a_weather_reading_change_sets_stale_through_the_frames_mirror`), not by a frame
- The clock page's weather line (shot 02) was seen at `f419a2f8` with a `weather` piece placed; it was not re-shot with the author's config after the fix
- A second output, a narrow output and a dark theme were not seen
- The clock page's tabs and the other Island pages were not pressed

## Checks

- `cargo fmt --all -- --check`: exit 0
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: exit 0
- `cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings`: exit 0
- `cargo test --workspace --locked`: exit 0; 46 `test result` lines summed: 2537 passed, 0 failed, 2 ignored
- the three guards: see the commit's report
- after the fix: build, nested probe and `smoke.sh --only bar,glance` re-run at `9d8b9097`; the checks above are from `f419a2f8` and the fix commit's own report

## Review

- **Simplify:** in the session; the capsule's view and the glance change needed no change.
- **Final review:** `/code-review` at high effort on Opus (the implementers were Sonnet and Haiku): one important finding and five nits. The weather worker now runs whenever a location is set, which ADR 0016 §3 did not allow: the ADR carries a dated amendment (the design §4.5 the author approved puts the weather in the capsule). The nits: stale comments and test names of the old rule, a test of the rebuild on a new reading (`a_weather_reading_change_sets_stale_through_the_frames_mirror`), an evidence line, the ledger row, and the capsule click test (from rest, `island toggle` reopening the last page, a sheet open then the capsule). Fixed in `6740756d` and `ae668396`.
- **Round 2:** the controller read the fixes and checked the new rebuild test by mutation (without `stale = true` it fails; restored, it passes). Clean.
- Tests after the fixes: 2539 passed, 0 failed, 2 ignored.

## Landing

- **Base revision:** `da2f6a2a0f3ae9fcd0c0cb100d6f84a8ed3b8d46`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,glance only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:1a797390aa6098107dac18ec67db30187665874134110ae948019b864c7bbbb4, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cd2b690f226026b1c73fede884cc6a483a2d3252cb8b6dce1827efb42302a050, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
