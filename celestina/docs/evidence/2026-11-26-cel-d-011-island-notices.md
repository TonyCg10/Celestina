# Evidence: notifications and the level display come out of the Island (CEL-D-011)

- **Date:** 2026-11-26
- **Scope:** `CEL-D-011`: `celestina/src/notifications`, `celestina/src/island`, `celestina/src/osd`, `celestina/src/app.rs`, `OsdPosition` in `celestina-rs/crates/celestina-shell/src/settings.rs`, their tests, the ledger row and `VAL-CEL-D-011`
- **Environment:** worktree `celestina-CEL-D-011` from `main`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** variant A of the Island notices preview, version 1, chosen by the author on 2026-11-26 (an artifact private to the author)

## Procedure

```sh
cd celestina
CARGO_TARGET_DIR=<private dir> cargo test -p celestina -p celestina-shell   # 2728 passed, 0 failed, 1 ignored
CARGO_TARGET_DIR=<private dir> cargo clippy -p celestina -p celestina-shell --all-targets -- -D warnings   # clean
CARGO_TARGET_DIR=<private dir> cargo clippy -p celestina -p celestina-shell -- -D warnings                  # clean
CARGO_TARGET_DIR=<private dir> cargo build --bin celestina --example fake_system_bus -p celestina-lock -p celestina-lock-verify
sh scripts/smoke.sh --binary <private dir>/debug/celestina --only notify,bar,osd
```

The counts are the sum of every `test result` line of the first command.

The captures ran in the guarded kit: `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl` and `pactl`, the host-wide nest lock, a nested niri at scale 1.5 on a 2560 by 1440 logical output (shots 3840 by 2160, cropped around the Island at physical pixels). Probes sent notifications with `notify-send`, moved the pointer with a virtual pointer client (`vp move`, `click`, `click right`, `click middle`, `axis`) and took shots in a background loop with `grim`, each stamped with its start time. The level display ran with `"osd": { "position": "island", ... }` in the kit's copy of `shell.json` only.

## Result

- **Tests:** 2728 passed, 0 failed, 1 ignored; clippy clean with and without `--all-targets`.
- **Smoke** (`--only notify,bar,osd`, exit 0, binary of `d45685a3`):
  - `smoke: osd on: surface created and destroyed, osd_frames=165, volume up reached wpctl as set-volume 0.60`
  - `smoke: notification 1 raised the banners' surface and it expired`
  - `smoke: winit frames=340 rest_frames=1 ... (frames=911 rest_frames=1 over 2 surfaces)` (the notify phase allows 2)
  - `smoke: bar on: winit frames=189 rest_frames=0 ...`
  - `smoke: OK (only: osd,notify,bar)`
- **Logs of the capture runs:** every burst logged `banners surface requested ... created=true` and, once its notices were gone, `banners surface released: no toast left`; the level alone logged `notices surface requested for a level created=true` and the same release line. The notices surfaces had 0 and 1 rest frames in the one run, the panel 2.
- **Placement:** the first captures of this unit showed every notice 51 px too low (the surface sat under the panel's exclusive zone), a page card narrower than its page, and a notice growing over a page that was still closing. They were fixed in `4fba99fa` and `d45685a3`, and every capture below was taken again on `d45685a3`: the Island and the notice keep their top at y 8 from the first frame, through the grow and the shrink, with no step, and a single Island is on screen at each step.
- **Captures** (`2026-11-26-cel-d-011-island-notices/`, 1100 by 640 physical, from x 1370; the Island centre is at x 550 in each):
  - [rest.png](./2026-11-26-cel-d-011-island-notices/rest.png): the clock Island at rest, y 8.
  - [notice-310ms.png](./2026-11-26-cel-d-011-island-notices/notice-310ms.png), [notice-400ms.png](./2026-11-26-cel-d-011-island-notices/notice-400ms.png), [notice-settled.png](./2026-11-26-cel-d-011-island-notices/notice-settled.png): a notice with two buttons; shot starts about 310 ms, 400 ms and 0.8 s after the post (the shots before them still show the clock). The first shows the grown shape with its face still empty, the second the text blurring in, the third sharp. Width 420, height 120, top at y 8.
  - [shrink-150ms.png](./2026-11-26-cel-d-011-island-notices/shrink-150ms.png) and [shrink-end.png](./2026-11-26-cel-d-011-island-notices/shrink-end.png): right click closing it; the shape shrinks towards the Island's size at y 8 with its text clipped and blurring out, then the clock pill takes over at the same place.
  - [burst-sheets.png](./2026-11-26-cel-d-011-island-notices/burst-sheets.png): three notifications, the newest with two sheets under it, each narrower than the one above.
  - [burst-fan.png](./2026-11-26-cel-d-011-island-notices/burst-fan.png): pointer over the notice: three full notices, 8 px apart.
  - [right-click-closes-front.png](./2026-11-26-cel-d-011-island-notices/right-click-closes-front.png), [wheel-up-closes.png](./2026-11-26-cel-d-011-island-notices/wheel-up-closes.png), [middle-click-closes-all.png](./2026-11-26-cel-d-011-island-notices/middle-click-closes-all.png), [burst-end.png](./2026-11-26-cel-d-011-island-notices/burst-end.png): the front one closes, then the next, then middle click closes the rest and the clock returns.
  - [level-grow.png](./2026-11-26-cel-d-011-island-notices/level-grow.png), [level-settled.png](./2026-11-26-cel-d-011-island-notices/level-settled.png): three volume steps with `osd.position: "island"`; the 274 by 40 level (icon, track, percentage) at y 8, and the empty face mid-grow.
  - [level-over-notice.png](./2026-11-26-cel-d-011-island-notices/level-over-notice.png): a level arriving over a notice; the notice's face leaves and the level fades in.
  - [page-without-card.png](./2026-11-26-cel-d-011-island-notices/page-without-card.png) and [page-card.png](./2026-11-26-cel-d-011-island-notices/page-card.png): the Clock page, then with a notice: the card (396 wide in the 420 page, 112 high with one button) sits at the top; its text and button fit; the page grew by its height plus 8.
  - [page-closing-toast-live.png](./2026-11-26-cel-d-011-island-notices/page-closing-toast-live.png) and [page-closed-toast.png](./2026-11-26-cel-d-011-island-notices/page-closed-toast.png): the page has closed to the clock pill (blurring face) with nothing over it; the notice grows only after, and then stays.
  - Clicking the Island at y 27 while a notice is out hits the notice body and opens the Notifications page (the probe's last two shots); no card is drawn there, as designed.

## Limits

- Fake system services and one output; the author's three monitors, the focus move between them, a fullscreen window, a busy background for the blur and reduced motion were not run: `VAL-CEL-D-011`.
- The shots start every 80 to 170 ms, so the 120, 240 and 400 ms frames of the plan were not hit: the file names are the shot's start after the post, not the frame's own time. In this run the first grown frame came about 300 ms after the post (the notify-send start counts in it); in an earlier run of the unfixed build it came at about 120 ms.
- The theme has no `telegram` icon in this environment, so that notice shows no icon.
- A test of the full suite failed once in two earlier runs of D11.1 and D11.4 and passed on rerun; it was not identified. It did not fail in this run.
- Frames with two Islands for one loop turn (D11.2, D11.4) were not seen in these captures but cannot be excluded at 80 to 170 ms between shots. The lent panel's bell badge now updates while the capsule is steadily lent; it still waits for the lend or return that is on its way.
- The review round fixes (the level display takes no input, a level waits for a closing page, one scroll closes one notice, a card's hold ends with its output, and the minor ones) are covered by unit tests only. Nothing of them was run in a nest: the 300 ms quiet time that ends a scroll gesture is a guess, since the toolkit sends no axis-stop event, and no real touchpad was used. A level surface left on an output the focus has since left still yields without a shrink when a notification arrives on the focused one.
- The tests ran at the final HEAD of the review round, on the same tree as clippy and fmt, from `celestina/`:
  - `cargo fmt --all -- --check`: clean.
  - `cargo test --workspace --locked`: 2798 passed, 0 failed, 2 ignored.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
  - `python3 scripts/check-language-contract.py` and `bash scripts/check-documentation-contract.sh`: OK.
  - The smoke and the captures ran on `d45685a3`, before the review round; they were not repeated.
- `providers::melibea::tests::a_focused_minimize_is_confirmed_by_the_window_niri_resolved` (`melibea.rs:1766`, outside this unit) is a known flaky test: it can fail in a full-workspace run and pass on rerun. It did not fail in the final run above.
- `connectivity::tests::failed_retries_start_no_watcher_and_a_lane_keeps_one_for_life` (`connectivity/mod.rs:1221`, outside this unit) failed once in the review's second round and once in the controller's last run, and passed on the rerun: another flaky test from `main`.
- After the second review round the controller made the wheel's gesture end with time only (every axis event on the surface feeds it, so the layout moving under a still pointer cannot end it) and let a press on a collapsed sheet click it when the fan gives it content before the release. Last run on that tree: `cargo fmt --all -- --check` clean, `cargo test --workspace --locked` 2799 passed and 0 failed (on the rerun), `cargo clippy --workspace --all-targets --locked -- -D warnings` clean.

## Landing

- **Base revision:** `cc715f7bb6d09f16d0ccf4f519fe5056944f2cf5`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:05d9483d73e7232a099c40f284148780c7a46e59fc24c8b604030ed63d429478, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2f5e646d03727bb412942dace26cffcb821d632aab80ca917e2f5a53c942b127, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
