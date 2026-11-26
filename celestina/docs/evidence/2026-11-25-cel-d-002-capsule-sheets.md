# Evidence: the capsule sheet and the volume sheet (CEL-D-002, Part A)

- **Date:** 2026-11-25
- **Scope:** `CEL-D-002`, Part A of the capsule sheets plan: the open-sheet owner (`src/sheet/`), its Island wiring (`src/island/sheets.rs`), the painter's `translate`, `cut_row` and `spotlight`, the volume menu as the first sheet body, and the volume popup removed from the app and the level controls. Tasks A1 to A5 are committed (HEAD `fca8aa04`); this file is task A6
- **Environment:** worktree `celestina-CEL-D-002` at `fca8aa04`; debug build in a private target directory; nested niri at 2560x1440 logical, scale 1.5 (3840x2160 pixels), output `winit`; the author's layout (`do_not_disturb` on, wallpaper for `winit`); fake `wpctl` and `pactl` (volume 0.40, two sinks, one source, Zen and Spotify streams); `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, `--own-bus`. With no bus and no DDC the end capsule shows sysmon, volume, clipboard and session
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Design and plan:** [design](../../../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md), [plan](../../../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md)
- **Preview:** approved by the author on 2026-11-25 (version 7 of the preview page). Its frames, 800 px wide, are kept in [preview](2026-11-25-cel-d-002-capsule-sheets/preview/): [volume sheet](2026-11-25-cel-d-002-capsule-sheets/preview/01-volume-sheet.jpg), [session sheet with the spotlight](2026-11-25-cel-d-002-capsule-sheets/preview/02-session-sheet-spotlight.jpg), [workspaces list](2026-11-25-cel-d-002-capsule-sheets/preview/03-workspaces-list.jpg), [centre capsule](2026-11-25-cel-d-002-capsule-sheets/preview/04-centre-capsule.jpg), [cut-edge variants](2026-11-25-cel-d-002-capsule-sheets/preview/05-cut-edge-variants.jpg). Only the first two concern Part A

## Procedure

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
CARGO_TARGET_DIR=<private tgt> cargo build --locked --workspace --bins
timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina --only bar,osd
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

The nested shots came from the capture kit's pattern (host-wide nest lock, `env -i`, fakes first on `PATH`, `dev-session.sh --own-bus --once --probe`), the binary pointed at the private build, pointer input from a virtual pointer (`vptr.py`, extended in the copy with a wheel and a drag), and `niri msg action screenshot-screen` for each frame, taken at least 400 ms after a click unless the frame is an opening in flight.

## Result

- **Exit:** `cargo test --workspace --locked` exits 0: 46 suites, 2490 passed, 0 failed, 2 ignored (every `test result` line summed). The smoke exits 0 and prints `smoke: OK (only: bar,osd)`. fmt, both clippy passes and the three guards were run by tasks A1 to A5 on their commits and by the final commit of this task (see Landing)
- **Smoke, rest frames:** `bar on: winit frames=79 rest_frames=0 n=77 late=0 comp_late=0 cpu_p50_ms=0.352 cpu_p99_ms=11.302 cpu_max_ms=11.302 (rest_frames=0)`; `rss_mib=111.2`; phase `osd`: `osd_frames=47`, `volume up reached wpctl as set-volume 0.60`, `osd off: both verbs rejected, no tool called`. The smoke never opens a sheet, so these counts are the bar at rest, unchanged
- **No new warning:** the nest's `run.err` has no line about the painter, an image, a texture, the spotlight or the cut row; the only non-environment warning is `idle: the worker did not take the command: the idle worker is still applying earlier commands`, which comes from the idle duty, not from a sheet
- **The look, read frame by frame** (crops of the top right, 1200 px wide; each file is under [the evidence directory](2026-11-25-cel-d-002-capsule-sheets/)):
  1. [volume sheet open](2026-11-25-cel-d-002-capsule-sheets/01-volume-sheet.jpg): the end capsule has grown down into one rounded panel, 480 px wide and 761 px tall at scale 1.5 (about 507 logical px). The row of icons stays on top in the same place (sysmon, volume with its pill, clipboard, session) and the body starts right under it: the volume slider with its mute switch, the folded output section showing the current device, the microphone slider with its mute switch, the folded input section, and the applications section with Zen and Spotify. Same layout as the approved preview frame `01-volume-sheet.jpg`
  2. [cut edge, zoomed 4x](2026-11-25-cel-d-002-capsule-sheets/02-cut-edge-zoom.jpg): the sysmon text "CPU 63 %" is cut by the sheet's left edge. It is blurred and fades toward the edge, and it sharpens to a clean "RAM 45 %" a few glyphs to the right. No seam, no halo, no dimmed band where the window meets the panel's rounded corner. Here only the left side cuts: the capsule's right end sits on the margin, so the cut on both sides of the plan is not reached by this layout
  3. [spotlight, zoomed 3x](2026-11-25-cel-d-002-capsule-sheets/03-spotlight-zoom.jpg): a soft light cone falls from the volume piece's pill down into the body, widening as it falls and fading out before the slider; no hard edge. It is faint, like the preview's session sheet spotlight (`02-session-sheet-spotlight.jpg`)
  4. [opening in flight](2026-11-25-cel-d-002-capsule-sheets/04a-open-in-flight.jpg) and [nearly settled](2026-11-25-cel-d-002-capsule-sheets/04b-open-nearly-settled.jpg): the shape grows first (a blank dark panel 316 px and then 746 px tall), and the body fades in over the last stretch (body at about 60 % alpha at 758 px of 761). The spotlight is already there in the second frame, not in the first
  5. [every section unfolded](2026-11-25-cel-d-002-capsule-sheets/05a-six-apps-unfolded-top.jpg), [scrolled to the middle](2026-11-25-cel-d-002-capsule-sheets/05b-six-apps-scrolled-mid.jpg), [scrolled to the end](2026-11-25-cel-d-002-capsule-sheets/05c-six-apps-scrolled-end.jpg): the sheet stops at its 544 px cap (a 816 px tall panel) and the body scrolls under the row, clipped by the rounded viewport with no content showing over the row. This used a fake graph with six application streams (Zen, Spotify, Firefox, Mpv, Discord, Telegram); with the two-stream graph every section unfolded still fits in 541 logical px and does not scroll
  6. [the bar at rest after closing](2026-11-25-cel-d-002-capsule-sheets/06-bar-at-rest-after-closing.jpg): the capsule is back to its size, text sharp, no panel, no leftover spotlight, nothing dimmed. A column scan at x = 3400 shows only the bar's 71 px after Escape, after an outside press and after a press on a cut icon
- **Interaction in the nest:**
  - the wheel over the volume slider (three notches) reached the tool as `wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.34 -l 1.0` and did not scroll the body ([frame](2026-11-25-cel-d-002-capsule-sheets/07-wheel-over-slider.jpg))
  - a drag on the slider held for three seconds reached the tool as `wpctl set-volume @DEFAULT_AUDIO_SINK@ 1.00 -l 1.0`; the [whole-screen frame during the drag](2026-11-25-cel-d-002-capsule-sheets/08-drag-no-osd-whole-screen.jpg) shows no on-screen display anywhere
  - pressing the volume piece a second time, `celestina island close` (Escape's verb) and a pointer press at (1280, 1300) each closed the sheet and left the bar at rest
  - pressing the cut sysmon icon with the sheet open closed the sheet and left the bar at rest with sysmon hovered ([frame](2026-11-25-cel-d-002-capsule-sheets/09-cut-icon-press-closes-sheet.jpg))

## Review

- **Simplify:** in the session, on the production code: the four live-anchor lookups of the sheet wiring became one helper (`b57a26fb`).
- **Final review:** `/code-review` at high effort on Opus (the implementers were Sonnet). Two important findings and four nits, all fixed in `b7df284c`: a sheet waiting for the page was dropped by the focus loss the Island itself caused when it dropped the keyboard; `Controls::open` released the held control before its own checks could refuse; the popup-closed path could release a held sheet; `Sheets::open` could drop a waiting page; the spotlight image was rebuilt every frame of a slide (now keyed on 8 device px steps); `geometry::hit` lost its unused capsule parameter.
- **Round 2 (re-review of the fixes):** the six are fixed; no new important finding. Two nits are left as follow-ups below. Tests after the fixes: 2493 passed, 0 failed, 2 ignored.

## Limits

- **Opening overshoot not seen.** About 40 opening frames were taken, at delays from 0 to 190 ms after the click; the screenshot call's own latency varies by 100 ms or more, so no frame landed at the 175 ms point. The tallest frame at rest height is 772 px against a settled 771 to 773 px. The OutBack overshoot is covered by the geometry and clock unit tests only
- **The body fades in after the shape.** Frames in flight show a blank dark panel until the last third of the motion. The preview page's own timing was not measured against this
- **Cut on both sides not shown.** The end capsule touches the right margin in this layout; a cut on both sides needs a capsule in the middle of the bar
- **Only the volume piece has a sheet in Part A.** A cut icon that asks for its own sheet (the "cut icon still opens its menu" of `VAL-CEL-D-002`) was not exercised: the cut icon pressed here is sysmon, which acts and closes the sheet
- **One output.** The nest has a single output (`winit`): opening the menu on a second output, and the multi-output release, were not seen. The unit tests cover `Controls::open` releasing across surfaces
- **Escape by verb, not by key.** The harness has no keyboard injector; Escape was `celestina island close`, which shares the close path but not the key event
- **No real monitor, no fractional scale.** Only scale 1.5 on the nested output was seen
- **GPU cost not measured.** The spike's `glFinish` timer is not carried over; the smoke's CPU figures above include no open sheet
- **Frame counts while a sheet is open** were not counted by the smoke (it never opens one). The unit test `an_open_sheet_at_rest_asks_for_no_frame` is the only check of "an open sheet at rest asks for no frame"
- The fake graph, volume and mute states are the capture kit's, not a real PipeWire session

## Follow-up

- `Sheets::open` leaves a waiting page queued behind a sheet it opens; the Island never reaches that today (review nit).
- The test of a refused level open checks the precheck only, not the order inside `Controls::open` (review nit).
- Tint the hover and press fills of the bar's pieces with the wallpaper's colours (the author's idea of 2026-11-25): a later `CEL-D` entry, recorded in the Backlog of the [CEL-TRACKS ledger](../plans/active/2026-11-25-cel-tracks.md)

## Landing

- Not landed by this task; the controller lands Part A with `land-unit.py --smoke bar,osd`.

## Landing

- **Base revision:** `8677a62bd745aea3a85b36ff988bacb4f26b16e8`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:98509d3a706c55b731f8bc0c34150c9b6da2183195cfaafac31e9373558186ed, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
