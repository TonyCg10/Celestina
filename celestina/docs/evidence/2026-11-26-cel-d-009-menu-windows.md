# Evidence: each capsule menu as its own window (CEL-D-009)

- **Date:** 2026-11-26
- **Scope:** `CEL-D-009`: `celestina/src/sheet`, `celestina/src/island`, the pages a page-backed window measures, their tests, the ledger row and `VAL-CEL-D-009`
- **Environment:** worktree `celestina-CEL-D-009` from `main`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** version 2 of the menu-window preview (variant B, lined up with its capsule), approved by the author on 2026-11-26 (an artifact private to the author)

## Procedure

```sh
CARGO_TARGET_DIR=$SCRATCH/tgt cargo build --workspace --bins
CARGO_TARGET_DIR=$SCRATCH/tgt cargo build --example fake_system_bus --locked --offline
scripts/smoke.sh --binary $SCRATCH/tgt/debug/celestina --only bar,osd,notify
```

The nested look ran through a guarded kit (`env -i`, `--own-bus`,
`CELESTINA_DDC=1` with a fake `ddcutil` that reads 60 % on the nest's own
output, fake `wpctl`, `pactl` and `brightnessctl`, the private fake system bus
of `examples/fake_system_bus` for NetworkManager, BlueZ, UPower and
power-profiles-daemon, nested niri on a 2560 by 1440 logical output at scale
1.5, the author's own bar layout). Every helper that talks to the nest calls
`nest_guard` first; the shell and the pointer ran only inside the nest. A
probe moved a virtual pointer (`zwlr_virtual_pointer_manager_v1`, raw Wayland
protocol, stdlib Python) and took full-output shots with `grim` against the
nest's display, in bursts about 95 ms apart right after a press. The shots are
cropped to the window area; every crop was read.

## Result

- **At rest, end capsule, lined up with its right edge:** [energy](./2026-11-26-cel-d-009-menu-windows/rest-energy.jpg) 240 by 228 px (logical), clearly narrower than 420, no empty area under the last row (the same bottom padding as the top); [Bluetooth](./2026-11-26-cel-d-009-menu-windows/rest-bluetooth.jpg) 262 by 228 (switch, toggle, two devices, the edit row); [brightness](./2026-11-26-cel-d-009-menu-windows/rest-brightness.jpg) 320 by 88, the level and the slider; [system load](./2026-11-26-cel-d-009-menu-windows/rest-sysmon.jpg) 360 by 268 (it declares its width, as the approved preview shows it; retaken after the fix, the sparkline fits inside), with the subtitle, the CPU and RAM graphs, the temperature and the open-monitor row. All four tops are at y 56 and their right edges sit on the capsule's right edge (the capsule is narrower while its piece is open; a window wider than it grows inward)
- **Start capsule:** [notifications](./2026-11-26-cel-d-009-menu-windows/rest-notifications.jpg) 240 by 156, its left edge on the capsule's left edge
- **Growing out of the piece:** [open-mid](./2026-11-26-cel-d-009-menu-windows/open-mid.jpg) six frames from the press on the energy piece: the frame before it, a nearly full window already fading in, then the settled one; [open-mid-early](./2026-11-26-cel-d-009-menu-windows/open-mid-early.jpg) frames taken earlier in the motion (the press issued 0 to 120 ms after the capture started): a small rounded rectangle at the piece's slot, the piece's icon inside it, growing toward the window
- **Switch:** [switch-mid](./2026-11-26-cel-d-009-menu-windows/switch-mid.jpg) six frames from the press on the Bluetooth piece with the energy menu open: in the second frame two windows show (the energy one folded smaller and darker toward its piece's slot, the Bluetooth one growing), then only the Bluetooth window, the energy piece back in the row
- **Close:** [close-mid](./2026-11-26-cel-d-009-menu-windows/close-mid.jpg) six frames from the press outside while the system load menu is open: the window folds toward the system load piece's slot at the left of the capsule (the text flies back to the slot), then the bar alone
- **Review:** 2 rounds (Opus, high; the implementers were Sonnet). Round 1: 0 critical, 1 important (the system-load and clock windows always 420: they now declare 360, the preview's width), 3 minor and 4 nits, fixed but one cosmetic nit. Round 2 re-read the fixes: 0 critical, 0 important, 1 minor (a window narrowed for a narrow output cut its body; the controller made it give up its margins first, test seen failing on the old code) and 1 nit, plus two parked items (Limits)
- **Exit:** smoke: OK (only: notify,bar,osd); the bar at rest with `rest_frames=0`, the notify run with `rest_frames=1`
- **Nest environment:** the energy, Bluetooth and network pieces show the fake system bus's states (a 64 % battery, the Balanced profile, one connected and one paired device, two Wi-Fi networks), not real hardware

## Limits

- Page heights are bounded by the pages' own row fit: a very long Bluetooth list stops below 496 px without scrolling.
- The natural width is measured unconstrained; content wider than 388 px gets 420 and is clipped. The system load and clock pages have no natural width (their sparkline and calendar cells are fixed parts), so they declare 360 through `PageView::sheet_width` and draw their parts in that width less the padding; every other page is measured, and none was compared with real-font widths in the nest.
- A window wider than the surface less its two 16 px margins gives up its margins first and keeps its width (420 on a 440 surface lands at x 10); only a window wider than the whole surface narrows, and its body clips on the right (tested; not shot).
- After review: when the open page is opened again during a fold, the windows still folding vanish at once; a page whose measured content is wider than the width it declares clips on the right without notice (no page does it today); the pill of a piece at the end of its row ends a few px off the gap it measures.
- The window is a glass shape on the panel surface, not its own Wayland surface (design 4.2, Claude's call).
- The shots show the shape of the motion, not its timing (the burst frames are about 95 ms apart and the capture instant inside a frame is unknown); the timing is asserted by tests. The look in the author's real session is left to `VAL-CEL-D-009`.
- Only the end capsule's right-edge rule and one start-capsule window were looked at; the centre-zone alignment, the narrow-output clamp and a scrolled tall window were not shot (tests only).
- The first frame of each burst is before the press and shows the piece's hover plate.

## Follow-up

None.

## Landing

- **Base revision:** `bf3ff960c44b55cd77a04e62a1e7313c5ca218dd`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: artifact digest or set does not match the recorded build; artifact is not verified yet; run verify-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:d8bcf230b847be562c7179dc8bc6f020dbb86b27b58f0588860117703d3e4636, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
