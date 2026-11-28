# Evidence: a menu opens from its icon (CEL-D-017)

- **Date:** 2026-11-27
- **Scope:** `CEL-D-017`: `celestina/src/sheet`, `celestina/src/island`, their tests, the ledger row and `VAL-CEL-D-017`
- **Environment:** worktree `celestina-CEL-D-017` from `main`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** rule B of the menu placement preview, version 1, chosen by the author on 2026-11-27 (an artifact private to the author)

## Procedure

```sh
cargo test --workspace --locked
CARGO_TARGET_DIR=$SCRATCH/tgt cargo build --locked --bins --examples
scripts/smoke.sh --binary $SCRATCH/tgt/debug/celestina --only bar,osd,notify
```

The nested look ran through the guarded kit of `CEL-D-010`, `CEL-D-014` and `CEL-D-015` (`env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`, `pactl` and `brightnessctl`, the private fake system bus, one `timeout -k 60 2700`, the host-wide nest lock, one nest at a time; `nest_guard` before every helper). The nested output was 3840 by 2160 pixels at scale 1.5 (2560 by 1440 logical). Layout: the end capsule held the system load first, then network, Bluetooth, volume, clipboard, battery, power profile and session; the start capsule held the workspaces and the notifications bell. A virtual pointer pressed each piece in turn and `grim` took one shot at rest after each; every sheet was read and cropped to the bar and its window.

## Result

- **Tests:** `cargo test --workspace --locked`: 3036 passed, 0 failed
- **Smoke:** `smoke: OK (only: osd,notify,bar)`; notify `rest_frames=2` (late=0), bar `rest_frames=0`, osd `osd_frames=59`; exit 0
- **End capsule, first to last piece** ([sheet 1](2026-11-27-cel-d-017-menu-from-icon/rest-end-1.jpg), [sheet 2](2026-11-27-cel-d-017-menu-from-icon/rest-end-2.jpg)): the system load window (CPU, RAM) sits under its piece with its left edge on the capsule's left edge (the piece is the capsule's first and the capsule's padding is 16 px, so 16 px before the piece is the capsule's left edge) and ends well short of the capsule's right edge; the network and Bluetooth windows start about 16 px before their pieces and end short of the right edge; the volume, clipboard, battery and session windows reach the capsule's right edge (they are wider than the room right of their pieces, so they moved inward as far as they must). The session piece's window, the last piece, sits where it did before the unit
- **Start capsule** ([shot](2026-11-27-cel-d-017-menu-from-icon/rest-bell.jpg)): the bell's window ends on the capsule's right edge, 16 px after the bell piece (the bell is the last piece of the start capsule), under the capsule
- **A switch** ([sheet](2026-11-27-cel-d-017-menu-from-icon/switch.jpg)): the system load window at rest; then the window of the session piece mid-travel (blurred content, box moved to the right, the pill passing the power profile piece); then the session window at rest on the capsule's right edge; then the Bluetooth window under its piece; then the bar alone after a press outside

## Limits

- At 2560 logical width the windows of the system load, network and Bluetooth do not reach the capsule's right edge; from volume on, every end piece's window does. The shots show the difference between those two groups. The window keeps the x it had when it opened while a text in the bar changes width (a test drives a level from 40 % to 100 %); the nest did not drive that.
- The switch was caught by two `grim` shots about 120 ms apart; offsets were not logged. Its timing is covered by the clock tests of `CEL-D-015`.
- The smoke does not open a menu. The nest backdrop is flat, so the glass behind a window was not judged. Reduced motion, wheel and right click were not driven.
- The look in the author's real session is left to `VAL-CEL-D-017`.
- **Final review (Opus, high, `/code-review`), 2 rounds.** Round 1: 0 Critical, 1 Important, 3 Minor, 3 Nit; the Important (an open window followed its piece, so a digit change in an end piece slid it about 7 px at rest) was ruled by the controller: the window's place is decided when it opens or switches and held while it stays open. Fixed in `c553ca57` with the Minor and Nit items. Round 2: 1 Important (that hold also froze the window through a tray slide, leaving it outside its narrower capsule), fixed by the controller after the last round in the next commit: a slide carries the window with its piece, to the frame that settles it; test `a_tray_slide_carries_an_open_window_with_its_piece`, seen failing without the fix. One Nit (a stale test count) fixed here.
- **Left open:** a capsule that shrinks for another reason while a menu is open (not a slide) does not re-place the window until the next open or switch.

## Landing

- **Base revision:** `05cf33ed1fa1baf7e87210556996a4b742742d3a`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:a0552811b9411c9dafdce0021fb45c8594d7cc14cac482e1f75e279c5b761596, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
