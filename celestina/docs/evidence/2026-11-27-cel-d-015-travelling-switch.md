# Evidence: the travelling switch (CEL-D-015)

- **Date:** 2026-11-27
- **Scope:** `CEL-D-015`: `celestina/src/sheet`, `celestina/src/island`, their tests, the ledger row and `VAL-CEL-D-015`
- **Environment:** worktree `celestina-CEL-D-015`, stacked on `unit/celestina/CEL-D-014`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** "Celestina · la gota", version 3, approved by the author on 2026-11-27 (an artifact private to the author)

## Procedure

```sh
cargo test --workspace --locked
CARGO_TARGET_DIR=$SCRATCH/tgt cargo build --locked --bins --examples
scripts/smoke.sh --binary $SCRATCH/tgt/debug/celestina --only bar,osd,notify
```

The nested look ran through the guarded kit of `CEL-D-010` and `CEL-D-014`
(`env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake
`wpctl`, `pactl` and `brightnessctl`, the private fake system bus, nested niri
on a 2560 by 1440 logical output at scale 1.5, one `timeout -k 60 2700`, one
nest at a time). The end capsule held the system load, network, Bluetooth,
volume, clipboard, battery, power profile and session pieces; the start
capsule held the workspaces and the notifications bell. Every helper that talks
to the nest calls `nest_guard` first. A probe pressed pieces with a virtual
pointer while a loop of `grim` shots ran (about 85 ms each); the kit logged the
start and end of every shot and the end of every press. Offsets below are from
the press (taken as 62 ms before the pointer helper returned) to the start and
end of each shot. Every sheet was read; each tile is cropped to the end
capsule and its window (and, for the bell, the start capsule).

## Result

- **Tests:** `cargo test --workspace --locked`: 2952 passed, 0 failed; `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and the architecture, documentation and language contracts pass
- **Smoke:** `smoke: OK (only: notify,osd,bar)`; bar `rest_frames=0` (337 frames), notify `rest_frames=2` (535 frames, `late=0`), osd 486 frames; `exit 0`
- **Network to Bluetooth** ([sheet](2026-11-27-cel-d-015-travelling-switch/s1-net-bt.jpg)): +0 ms the network window with the pill on the network piece; +81..+168 ms the pill already on Bluetooth while the window still shows the network content; +168..+255 ms the content blurred out, the box narrowed on its left edge, the Bluetooth content blurred in; +256..+343 ms the box at its new size, the new content sharpening; +343 ms on the Bluetooth window sharp. One box on the capsule in every tile
- **Bluetooth to volume** ([sheet](2026-11-27-cel-d-015-travelling-switch/s1-bt-vol.jpg)): the same sequence: pill on volume from +51..+138 ms, old content gone by +138..+231 ms, the box widening to the left and growing taller, volume content blurred and sharpening by +231..+320 ms, sharp from +320 ms. One box in every tile
- **Volume to the bell** ([sheet](2026-11-27-cel-d-015-travelling-switch/s2-vol-bell.jpg)): +90..+168 ms the volume window blurring and the bell window a small dark box under the bell; +168..+254 ms the bell window larger and empty, the volume window a blurred shrinking box; +254..+340 ms the bell content faint, a small remnant of the volume box; from +341 ms the bell window alone and the end capsule bare. Two boxes for about 250 ms, one per capsule, then one
- **Bell to volume to bell, fast** ([sheet](2026-11-27-cel-d-015-travelling-switch/s3-bell-vol-bell.jpg)): the bell window blurs out and shrinks while a small volume window starts; the volume window grows with content, then shrinks again as the second press brings the bell window back from its shrunken shape, blurred first, sharp by about +300 ms; the end capsule ends bare. The last press wins, one window at the end
- **Volume to bell to volume, fast** ([sheet](2026-11-27-cel-d-015-travelling-switch/s4-vol-bell-vol.jpg)): a small bell window starts and is taken back; the volume window blurs and shrinks, then returns to full size and sharp by about +300 ms; the bell capsule ends bare. No second volume window in any tile
- **Reopen during a close** ([sheet](2026-11-27-cel-d-015-travelling-switch/s5-close-reopen.jpg)): the volume window at rest, then blurred and shrinking after the press outside, then (+107..+193 ms) smaller still with the content almost gone, then growing back from that shape with content blurred (+193..+284 ms) and sharp by +373 ms. One window in every tile; the pill stays behind volume throughout
- **At rest after each** ([montage](2026-11-27-cel-d-015-travelling-switch/rest-after-each.jpg)): the bar alone; the network window; the Bluetooth window; the volume window; the bell window; the bell window; the volume window; the volume window; the bar alone after the last close. Each is one window on one capsule with the pill on its piece (the bell on the start capsule), and no window left on the other capsule

## Limits

- The shots show the shape of the motion, not its timing: a `grim` shot takes about 85 ms and its capture instant inside that window is unknown, so offsets are windows, not instants. The 300 ms travel and its curves are asserted by the clock tests. The look in the author's real session is left to `VAL-CEL-D-015`.
- The sheets are the bursts' frames, not frames taken at fixed offsets; each scenario ran once, at one phase between the first shot and the press.
- The nest has no brightness piece (`CELESTINA_BACKLIGHT=0`) and no tray item, so the switch was shot network to Bluetooth and Bluetooth to volume, not through brightness or the chevron.
- niri's backdrop blur behind the window during the travel is seen only in the nest: the shots show the shell's own blurred content, and the glass behind the box is the nest's flat backdrop. How the glass looks over a real wallpaper is left to `VAL-CEL-D-015`.
- Reduced motion was not shot for the switch; the tests cover it.
- The wheel, right click and click on the open piece were not driven in the nest.
- "No frame is drawn at rest after a close" rests on the smoke's `rest_frames=0` for the bar (which opens no menu) and on the tests; no nest frame log after a switch was read. Notify's `rest_frames=2` is as in `CEL-D-014`.
- The first two attempts to start the nest ran with a probe file that was not executable, so the nest started without a probe; each was stopped by its own process id within seconds (the first run was a position-finding shot only). Nothing touched the host session.
- **Final review (Opus, high, `/code-review`), 2 rounds.** Round 1: 0 Critical, 1 Important, 4 Minor, 4 Nit. The Important (a menu opened while the Island page waited behind a closing sheet left the page queued, and it opened unasked when the menu closed), three Minor (a late reopen joined the blur region before its glass was back; the left level body was flushed 90 ms late; the reduced-motion wording of `VAL-CEL-D-015`) and one Nit were fixed in `6c9f0561`, each with a test seen failing first. Round 2: no findings.
- **Left open:** a close in the first 90 ms of a travel drops the old content in one frame (`CEL-D-013` replaces the close); the old content does not follow a sliding capsule; `drop_open` cuts every leaving window; no test covers `on_leaving` or a leaving window's region; the reduced-motion ordering fix has no test.

## Landing

- **Base revision:** `0f7e4cd1f6897629129df536bc469e66400f8d25`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:e57306501b70eb412a1286fed052252b9c2718db5d2f1c140ee37f05a4c7438c, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
