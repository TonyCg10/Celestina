# Evidence: the workspaces list (CEL-D-005, Part D)

- **Date:** 2026-11-25
- **Scope:** `CEL-D-005`, Part D, the last unit of the capsule sheets series: a press on the workspaces piece opens a list sheet with one row per workspace of the output, the icons of its windows and the current one marked; a click on a row focuses it and closes the list; the wheel over the piece steps workspaces, open or not. Tasks D1 and D2 are committed (`a819531c`, merged with `main` at `96be6351`); this file is task D3
- **Environment:** worktree `celestina-CEL-D-005` at `96be6351`; debug build in a private target directory; nested niri at 2560x1440 logical, scale 1.5 (3840x2160 pixels), output `winit`; the nest's niri profile is `scripts/dev-session.kdl` with the workspaces 7 to 15 removed, so six named workspaces ("1" to "6") plus niri's trailing empty one (the author's strip has six); `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, `--own-bus`, no system bus; `kitty` windows spawned inside the nest
- **Artifact:** the approved preview frame `03-workspaces-list.jpg` of the session's notes (the design session's preview); the shots below follow it
- **Design and plan:** [design](../../../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md), [plan](../../../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md)

## Procedure

```sh
CARGO_TARGET_DIR=<private tgt> cargo build --locked --bin celestina
CARGO_TARGET_DIR=<private tgt> cargo build --locked -p celestina-lock -p celestina-lock-verify
CARGO_TARGET_DIR=<private tgt> cargo build --locked --example fake-mpris-player
CARGO_TARGET_DIR=<private tgt> cargo build --locked --offline --example fake_system_bus
timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

The nested shots came from the capture kit (host-wide nest lock, `env -i`, fakes first on `PATH`, `dev-session.sh --own-bus --once --probe`), the binary pointed at the private build, pointer input from a virtual pointer, and `niri msg action screenshot-screen` for each frame, taken 1 to 2 s after the action. The windows: two `kitty` on workspace 1, one on 2, none on 3, one on 4, two on 5, none on 6 (workspace 7 is niri's trailing empty one). Probes talk to the shell only through the guarded `cel` function and to niri through the guarded `nmsg`. After each action the probe logged `niri msg workspaces` (the focused workspace) as the check of the effect. The crops are 600x120 and 600x600 pixels of the 3840x2160 frames.

## Result

- **Smoke (full, at `96be6351`):** `smoke: OK`, exit 0. The phases ran in the order launcher,energy,clipboard,glance,cutover,overview,notify,bar,osd,wallpaper,connectivity,phone,media,polkit (all fourteen). Lines: `bar on: winit frames=76 rest_frames=0`; `clipboard: winit frames=950 rest_frames=0`; `glance: frames=315 rest_frames=1 curl_calls=0`; `glance-weather: frames=330 rest_frames=1 curl_calls=1`; `cutover: ... winit frames=372 rest_frames=0`; `connectivity ... rest_frames=0`; `phone and bubbles ... rest_frames=0`; `media: ... rest_frames=0`; `wallpaper present, frames=75 rest_frames=0`; `osd on: surface created and destroyed, osd_frames=117`; the notify phase `frames=146 rest_frames=1`
- **The piece at rest** ([crop](2026-11-25-cel-d-005-workspaces-list/01-piece-at-rest.jpg)): the dots of the output's seven workspaces, the active one a blue pill, with no square under any dot
- **The piece under the pointer** ([crop](2026-11-25-cel-d-005-workspaces-list/02-piece-hover-pill.jpg)): one grey pill under the whole piece, not one square per dot
- **The list open** ([crop](2026-11-25-cel-d-005-workspaces-list/03-list-open.jpg)): the capsule grew into a sheet 240 wide under the piece; seven rows of 40 with their numbers; `kitty` icons (two on rows 1 and 5, one on rows 2 and 4); the empty-workspace label in the faint tone on rows 3, 6 and 7; the current row 1 in the soft accent fill with its border and its number in the accent. The piece keeps its grey pill while the sheet is open
- **A row under the pointer** ([crop](2026-11-25-cel-d-005-workspaces-list/04-list-row-hover.jpg)): row 4 takes the hover fill; the current row keeps its coloured fill
- **After a click on row 4** ([crop](2026-11-25-cel-d-005-workspaces-list/05-after-row-click.jpg)): the sheet closed (the capsule shrank back) and the fourth dot is the pill; `niri msg workspaces` went from `* 1 "1"` to `* 4 "4"`
- **The wheel, list closed** ([crop](2026-11-25-cel-d-005-workspaces-list/06-wheel-closed.jpg)): from workspace 4, two notches up gave workspace 2 (the log), then three quick notches down gave workspace 4 (see Limits); the active dot follows. Nothing opened
- **The wheel, list open** ([crop](2026-11-25-cel-d-005-workspaces-list/07-wheel-open.jpg)): one notch up from workspace 4 gave workspace 3; the sheet stayed open and the current row moved to row 3 (the soft accent fill, the number in the accent), the piece's pill moved to the third dot
- **Against the approved frame:** the row height, the number column, the icons, the empty label, the current row's fill with its border, the sheet's width and its join with the capsule match the approved frame; the frame's icons are the author's theme's, here the nest's `kitty` icon

## Limits

- The nest has `kitty` windows only: no icon fallback (`app-window`), no `+N` row (more than six windows), no floating windows and no named workspace were seen in a frame. The unit tests of `src/sheet/workspaces.rs` cover them
- The author's six workspaces were stood in for by six named workspaces; the real workspace names and window mix were not seen
- Row 7, niri's unnamed trailing workspace, showed its empty label about 9 logical pixels further right than rows 3 and 6 (named "3" and "6") in the shots: a short label now takes the 18 px column, named or not (the controller's simplify commit `9fba1188`). The shots predate that fix and were not retaken
- The virtual pointer's wheel tick sends a smooth 15 px axis and a discrete step together; the shell counted that as two workspaces per tick: a tool artifact (in the virtual-pointer protocol a discrete event carries its own value), not seen with a real wheel, whose notch is one frame. A smooth-only tick (what a touchpad sends) moved one workspace per tick. The shots use smooth-only ticks. Three smooth ticks 50 ms apart moved two workspaces (2 to 4), not three: a tick fired before the previous focus change reached the model was measured from the older workspace. The final review fix (round 1) keeps the last target of a burst (1500 ms) and steps from it; its unit test covers three notches 50 ms apart (3, 4, 5), the nest was not re-run. One tick per 1 s moved one workspace each time (2, 3, 4, then back 3, 2). A real wheel is slower than 50 ms between notches, but this was not measured on a real device
- A second output, a narrow output, a dark theme, the list taller than the cap (more than about 12 rows) and Enter on a focused row were not seen in frames
- No frame in flight: the shots are settled frames. The rest checks are the smoke's
- `cargo test --workspace --locked`, both clippy passes and `cargo fmt` were not re-run in this task (D2's report holds them); this task changed documents only

## Final review fix, round 1

- Wheel burst: a notch within 1500 ms steps from the last target asked, not the model's active workspace
- A queued workspaces sheet rebuilds its list when it opens
- At most six icons then `+N` (eight did not fit the 240 px row); the width stays
- A workspace name longer than 12 graphemes is cut with an ellipsis and its label capped at 96 px
- None of these was seen in a nest frame; the unit tests cover them

## Checks

- full smoke: `smoke: OK`, exit 0
- `bash scripts/check-architecture-contract.sh`: exit 0
- `bash scripts/check-documentation-contract.sh`: exit 0 after this file existed (two broken-link errors before it)
- `python3 scripts/check-language-contract.py`: exit 0
- tests, fmt and clippy: not re-run here; see D2's report

## Review

- **Simplify:** in the session: the label's role and text computed once, and short labels (a number, named or not) in the 18-px column so the rows' icons line up (`9fba1188`, with a test checked by mutation).
- **Final review:** `/code-review` at high effort on Opus (the implementers were Sonnet): the double step per wheel notch seen in the nest came from the test tool (it sent both a smooth and a discrete value); the shell uses the smooth value only. Three important findings, fixed in `d42e3035`: fast notches lost steps (a 1.5 s burst now steps from the last target asked); a queued list opened stale; eight icons did not fit the approved 240-px row (now six, then `+N`, the controller's call to keep the approved width). Nits fixed: long names are cut at 96 px; the evidence, STATUS and ROADMAP lines. Parked: the per-event rounding of the bar's wheel steps (pre-existing, also the volume's), a cap on the steps of one event, the 18-px column for two-character labels.
- **Round 2:** the controller read the fixes (the burst resets once the model shows the target or after 1.5 s; the test covers three notches and the return to the model). Clean.
- Tests after the fixes: 2559 passed, 0 failed, 2 ignored.

## Landing

- **Base revision:** `c75371c8c7156bb4463da6ef118c794e800efac7`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7b92898baa01878b893b5a12cce9fe6ed9994bfbeac857f5942550130719023e, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
