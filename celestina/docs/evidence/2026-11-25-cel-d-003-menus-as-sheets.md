# Evidence: every piece's menu as a capsule sheet (CEL-D-003, Part B)

- **Date:** 2026-11-25
- **Scope:** `CEL-D-003`, Part B of the capsule sheets plan: the brightness, session and page menus of the bar's pieces open as capsule sheets; the level popup and the session menu `xdg_popup` paths are removed; the Island keeps its pages for `island open`, its tabs and notifications. Tasks B1 to B3 and the notifications-toggle fix are committed (HEAD `806d15c9`); this file is task B4
- **Environment:** worktree `celestina-CEL-D-003` at `806d15c9`; debug build in a private target directory; nested niri at 2560x1440 logical, scale 1.5 (3840x2160 pixels), output `winit`; the author's layout with `do_not_disturb` on; fake `wpctl` and `pactl` (volume 0.40, two sinks, one source, Zen and Spotify streams); a fake `ddcutil` that detects three displays and reads 40, 65 and 80; `CELESTINA_DDC=1`, `CELESTINA_BACKLIGHT=0`, `--own-bus`, no system bus. The end capsule shows sysmon, volume, brightness, clipboard and session
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Design and plan:** [design](../../../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md), [plan](../../../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md)
- **Preview:** approved by the author on 2026-11-25 (see the [CEL-D-002 evidence](2026-11-25-cel-d-002-capsule-sheets.md)); this unit applies the same sheet to more content and adds no new look

## Procedure

```sh
CARGO_TARGET_DIR=<private tgt> cargo build --locked --bin celestina
CARGO_TARGET_DIR=<private tgt> cargo build --locked -p celestina-lock -p celestina-lock-verify
CARGO_TARGET_DIR=<private tgt> cargo build --locked --example fake-mpris-player
CARGO_TARGET_DIR=<private tgt> cargo build --locked --offline --example fake_system_bus
timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina --only bar,osd,glance,connectivity,energy,clipboard,media,phone,notify
cargo test --workspace --locked
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

The nested shots came from the capture kit's pattern (host-wide nest lock, `env -i`, fakes first on `PATH`, `dev-session.sh --own-bus --once --probe`), the binary pointed at the private build, pointer input from a virtual pointer (`vptr.py`), and `niri msg action screenshot-screen` for each frame, taken 2 s after a click. Probes talk to the shell only through a guarded `cel` function that refuses to run outside the nest.

## Result

- **Exit:** `cargo test --workspace --locked` exits 0: 46 suites, 2518 passed, 0 failed, 2 ignored (every `test result` line summed). The smoke exits 0 and prints `smoke: OK (only: glance,energy,osd,notify,bar,connectivity,phone,clipboard,media)`; the media phase ran (`media: paused player with a cover, page open, rest_frames=0`), it was not skipped. The checks of tasks B1 to B3 (fmt, both clippy passes) ran on their commits; this task changes no code
- **Smoke, rest frames:** `bar on: winit frames=72 rest_frames=0 … cpu_p50_ms=0.367`; `connectivity … rest_frames=0`; `phone and bubbles on … rest_frames=0`; `clipboard … rest_frames=0`. The `glance` and `notify` phases print `rest_frames=1` (`glance: frames=130 rest_frames=1`; `notify: frames=130 rest_frames=1 over 2 surfaces`) and pass their own check. The smoke never opens a sheet, so these are the bar at rest
- **The look, read frame by frame** (crops of the top of the screen, each file under [the evidence directory](2026-11-25-cel-d-003-menus-as-sheets/)):
  1. [brightness sheet](2026-11-25-cel-d-003-menus-as-sheets/01-brightness-sheet.jpg): the end capsule has grown into a 480 px (320 logical) panel under its row of pieces. The brightness piece is lit with the spotlight under it. The body is one row, the brightness label and "40 %", with a slider at 40 %. This is the single-monitor body (see Limits)
  2. [session sheet](2026-11-25-cel-d-003-menus-as-sheets/02-session-sheet.jpg): 360 px (240 logical) wide, narrower than the capsule, so the row stays where it was and the sysmon and volume pieces left of the window are blurred out (the volume text is blurred at the left edge). Rows: Lock, Suspend, a divider, Log out, Restart, Shut down, each with its icon. The power piece is lit
  3. [confirm card](2026-11-25-cel-d-003-menus-as-sheets/03-session-confirm-card.jpg): pressing Restart replaces its row by a card asking whether to restart the machine now, with a Cancel button (focus ring) and a red Restart button. The sheet grew down by the card's height; the other rows kept their places and nothing ran. Neither button was clicked
  4. [System sheet](2026-11-25-cel-d-003-menus-as-sheets/04-system-sheet.jpg): the narrow-capsule case. The sheet is 630 px (420 logical) wide and holds the whole capsule, so the row is unchanged and the sysmon piece is lit. Title (System) with the overflow button ("More") on the right, CPU and RAM with their bars, the temperature (57 °C) and the row that opens the system monitor. There is no clock, no "Cerrar" and no tab row
  5. [Notifications sheet](2026-11-25-cel-d-003-menus-as-sheets/05-notifications-sheet.jpg): from the start capsule, the wide case. The workspaces dots and the bell stay in their row, the capsule grew to 630 px (420 logical) wide and the panel below holds the title (Notifications), the overflow button, the Do Not Disturb switch (on), the clear-all button and four notification cards ("Screenshot captured", from the harness's own screenshots) with their close buttons. The bell is lit
  6. [overflow menu](2026-11-25-cel-d-003-menus-as-sheets/06-overflow-menu.jpg): pressing the overflow button on the System sheet opens a small panel under the button, over the sheet: reopen the page, Do Not Disturb with its switch, quit. The sheet behind it stays open and the menu's own panel is blurred behind
  7. Tour, one frame per press from rest: [volume](2026-11-25-cel-d-003-menus-as-sheets/07-tour-1-volume.jpg) (volume piece lit; volume slider, mute switch, folded output with the device, microphone, input, applications Zen, Spotify muted, Firefox, Mpv; the body is cut at the 544 px cap), then a press on the visible part of the sysmon text [switches to the System sheet](2026-11-25-cel-d-003-menus-as-sheets/08-tour-2-system.jpg), then a press on the brightness piece [switches to the brightness sheet](2026-11-25-cel-d-003-menus-as-sheets/09-tour-3-brightness.jpg) (the panel shrank to 320 and kept the row), then a press on the power piece [switches to the session list](2026-11-25-cel-d-003-menus-as-sheets/10-tour-4-session.jpg), then a press on the desktop [closes it to the bar at rest](2026-11-25-cel-d-003-menus-as-sheets/11-tour-5-closed.jpg). Each settled frame shows exactly one panel and the Island pill is not open
  8. [another capsule](2026-11-25-cel-d-003-menus-as-sheets/12-other-capsule-opens-notifications.jpg): with the volume sheet open, a press on the bell closed it and opened the Notifications sheet; the end capsule is back at rest in the same frame, the date and clock capsules sit in the centre
- **Interaction in the nest:**
  - a press on the sysmon piece while the 240 px session sheet was open closed the sheet and did not open System: the piece is outside the window and not drawn, as the design says ("pieces outside the window are not drawn"). The press counted as an outside press
  - pressing Suspend in the session list ran the session's Suspend path at once, with no confirm card: the log shows `celestina_wayland::lock::handlers: the compositor confirmed the lock` then `session Suspend: unavailable: cannot reach the bus` (no system bus in the nest). That is the intended behaviour: only Log out, Restart and Shut down ask first (CEL-I-006)
  - the confirm card and the rows were pressed with the virtual pointer only; no keyboard was injected

## Correction: the focus indicator replaces the spotlight

CEL-D-002 shipped a spotlight cone under the open piece by a misreading of "foco"; the approved look is I5 "Foco" of the preview. This unit removes the spotlight (its module, `Painter::spotlight`, its cached image and clocks) and paints every other piece of the open capsule at 40 % opacity (200 ms in on `ONE_UI`, 150 ms out on `EXIT`, a switch keeps it, reduced motion jumps it). The open piece keeps the indicator pill. The nest was run for this change (build at 1aa5a371, one output, scale 1.5): [volume sheet open](2026-11-25-cel-d-003-menus-as-sheets/13-focus-volume-sheet.jpg), the brightness piece at about 40 % opacity and the cut sysmon and power pieces faded by the edge, the volume piece full with its grey pill and nothing under it, no cone; [session sheet open](2026-11-25-cel-d-003-menus-as-sheets/13-focus-session-sheet.jpg), the narrow case where the cut and the dim act together, the brightness and clipboard pieces dimmed, the power piece full with its pill; [about 1 s after closing](2026-11-25-cel-d-003-menus-as-sheets/13-focus-closed-1s.jpg), every piece back at full opacity. In one run the first press outside, right after a session sheet opened, did not close it (frames at 1 s and 4 s still open); the next outside press closed. Not investigated The frames above that show a spotlight (the CEL-D-002 evidence's `03-spotlight-zoom`, the volume-lit frames here) show the old look.

## Review fixes checked in the nest

Build at e948d7cb (sheets take the keyboard Exclusive while shown; an invisible `celestina-catcher` layer covers the output below y 560 so a press outside closes the sheet). One output (`winit`, 3840x2160 capture, scale 1.5), fake `ddcutil` (`CELESTINA_DDC=1`), no system bus. Each case started from rest. Pointer from the virtual pointer; the keys (Escape = evdev 1, Tab = evdev 15) from a scratch virtual keyboard (`zwp_virtual_keyboard_manager_v1`, US keymap), run only inside the nest probe. Crops are the top right of the capture.

1. Session sheet, one press at 1280,1300 (empty desktop): [open](2026-11-25-cel-d-003-menus-as-sheets/14-session-open.jpg), then [closed 1.2 s after the first press](2026-11-25-cel-d-003-menus-as-sheets/14-session-outside-press-closed-1s.jpg), bar at rest. Before the fix this took a second press. Passed
2. Volume sheet, Escape once: [open](2026-11-25-cel-d-003-menus-as-sheets/14-volume-open.jpg), then [closed 0.5 s after the key](2026-11-25-cel-d-003-menus-as-sheets/14-volume-escape-closed-1s.jpg). Closed on the first try. Passed
3. System sheet, Tab: [first Tab](2026-11-25-cel-d-003-menus-as-sheets/14-system-tab-1.jpg) puts the focus ring on the overflow button ("More"), [second Tab](2026-11-25-cel-d-003-menus-as-sheets/14-system-tab-2.jpg) moves it to the row that opens the system monitor. Escape then closed it ([3 s later, at rest](2026-11-25-cel-d-003-menus-as-sheets/14-system-closed-3s.jpg)). Passed
4. At rest after each close nothing is open (frames above). The shell log has a `layer surface created ... namespace="celestina-catcher"` line per open sheet (surfaces 4, 5, 6) but logs no destroy line at all (no destroy message exists in `runtime.rs`), so the destruction was read from the compositor: a second run called `niri msg layers` inside the probe. Rest: panel and wallpaper only. Each sheet open: plus one `celestina-catcher`. After the outside press and after each Escape: panel and wallpaper only, no catcher. Passed
5. Smoke: `timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <tgt>/debug/celestina --only bar,osd,notify,polkit` exited 0 and printed `smoke: OK (only: notify,bar,osd,polkit)`. The polkit phase ran (`polkit on over the dead system bus: the agent is unavailable and the shell did not panic`); `bar on ... rest_frames=0`, `osd` surface created and destroyed

Not checked here: the polkit prompt taking the keyboard back from an open sheet (no polkit agent on a dead bus), a second output, a press outside above y 560 other than on the bar.

## Review

- **Simplify:** in the session: the page sheets' local Escape keysym now uses the sheet wiring's one (`e000d687`).
- **Final review:** `/code-review` at high effort on Opus (the implementers were Sonnet): two important findings and four nits. A sheet opened from rest never had the keyboard, so Escape and a press below the panel surface did not close it (the Island page had the same gap on `main`); a queued Notifications sheet was never marked read. Round 1 (`57da31b8`, `8b78e5bf`): while anything is shown the panel takes the keyboard Exclusive, and a transparent `celestina-catcher` layer (layer Top, keyboard none, input below y 560 only) catches an outside press and closes what is shown; the nits fixed.
- **Round 2 (security):** round 1 had admitted the Island as a third Exclusive keyboard opener in the polkit guard `n25` without the Island checking the prompt's claim. `e948d7cb`: the Island takes the claim from the app, asks no keyboard, opens nothing and keeps no catcher while a prompt holds the keyboard; `n25` asserts the gate.
- **Round 3 (re-review, the last):** no security hole: the claim is set before the prompt card exists, a queued open is refused, niri serves Overlay before Top. One important finding fixed by the controller without another reviewer, under the author's rule after the last round (`ba52ffea`): after a prompt ended, `island open`, the toggles and the banner's open did nothing until the Island's next frame, because its claim flag was stale; the app now hands the Island the current claim before each verb. The ledger row gained `tests/polkit_guards.rs` and `src/lib.rs`.
- Tests after the fixes: 2531 passed, 0 failed, 2 ignored.

## Follow-up

- The polkit guard's check of the Island's gate is a text match: a refactor could keep the flag's name and drop its meaning (re-review nit).
- Between a sheet opening and the catcher mapping (one compositor round trip) a press below y 560 reaches the window under it (re-review nit).
- Each open allocates a full-output buffer for the empty catcher; an input-only surface would avoid it (re-review nit).
- The app paths that hand the Island the claim before a verb have no unit test: `App` cannot be built in tests.
- Section 4.4 of the design still describes the spotlight; a TRK-1 documents unit amends it to the focus indicator.

## Limits

- **Brightness shows one monitor, not three.** The fake `ddcutil` detects three displays and reads all three, but the card lists one row per niri output and the nest has one output (`winit`, matched through the fake connector `card1-winit`), so the single-monitor body is shown. The multi-monitor card of CEL-I-003 inside a sheet (the "All" heading, divider and per-monitor rows) was not seen; the unit tests of `osd/popup.rs` cover it
- **No frame in flight.** Switching between pieces was seen settled only. The screenshot call lags the click by well over 100 ms, so the one-motion morph of a switch (and the OutBack of the open) is covered by unit tests, not by a frame. One frame requested 250 ms after a click showed the sheet already switched with both pieces lit
- **Lock was not pressed.** Pressing Lock in the nest would lock the nest's session and hide every later frame. The lock path was reached through Suspend, which locks first (see above); `Lock locks` stays for `VAL-CEL-D-003`
- **The confirm card was not confirmed or cancelled.** Only its opening was seen
- **The overflow menu's rows were not pressed.** The reopen row (a sheet closes at once and grows again from its capsule) and the compositor dismissal closing the sheet were not exercised; one run's log shows `menu: dismissed by the compositor` after a press outside the menu
- **Page sheets other than System and Notifications** (clipboard, network, bluetooth, clock, media, phone) were not opened: their pieces are absent from the nest's bar (no bus) or not in the shot plan. The smoke phases for them (`clipboard`, `connectivity`, `media`, `phone`) pass but do not open a sheet
- **Escape by verb, not by key** in the first runs (no keyboard injector); the review fixes were checked with a real Escape and Tab, see "Review fixes checked in the nest"
- **One output, scale 1.5 only;** a second output and a narrow output were not seen
- **Notifications list.** The four cards are the nest's own screenshot notifications ("Screenshot captured"), not a fake feed
- **GPU cost and frame counts with a sheet open** were not measured; the smoke never opens a sheet

## Landing

- Not landed by this task; the controller lands Part B with `land-unit.py --smoke bar,osd,glance,connectivity,energy,clipboard,media,phone,notify`.

## Landing

- **Base revision:** `f24a1ed501bf98394789d0d7937137b88f21b9d3`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,glance,connectivity,energy,clipboard,media,phone,notify,polkit only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:d9251a51d39a2fe8188674ef4bd34d13e2b9addc2507082ab8940692711e74f0, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
