# Evidence: a spotlight on the wallpaper (CEL-D-018)

- **Date:** 2026-11-28
- **Scope:** `CEL-D-018`: `celestina/src/wallpaper`, `celestina/crates/celestina-paint`, their tests, the ledger row and `VAL-CEL-D-018`
- **Environment:** worktree `celestina-CEL-D-018` from `main`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the bar presence preview, version 6, variant W1 at 30 %, chosen by the author on 2026-11-28, private artifact <https://claude.ai/artifact/Rb2kceh7p66VCFYWQEHL34>

## Procedure

```sh
cargo test --workspace --locked
CARGO_TARGET_DIR=$SCRATCH/tgt cargo build --locked --bins --examples
scripts/smoke.sh --binary $SCRATCH/tgt/debug/celestina --only bar,wallpaper
```

The nested look ran through the guarded kit of the earlier `CEL-D` units (`env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`, `pactl` and `brightnessctl`, the private fake system bus, one `timeout -k 60 2700`, the host-wide nest lock, one nest at a time; `nest_guard` before every helper). The nest's own `shell.json` turned the wallpaper duty on and named the author's picture (`wallhaven-e8yvzl.jpg`, copied into the nest's own folder) for the nest's output `winit`. Two nests: one at scale 1.5 (3840 by 2160 pixels, 2560 by 1440 logical) for the rest, menu and full shots, one at scale 1.25 (3072 by 1728 logical) and one at 1.333 for the seam. A virtual pointer opened the volume menu; `grim` took the shots; every shot was read.

## Result

- **Tests:** `cargo test --workspace --locked`: 3130 passed, 0 failed (rerun, summed from the `test result` lines)
- **Smoke:** `smoke: OK (only: bar,wallpaper)`; bar `rest_frames=0`; wallpaper `frames=300 rest_frames=0` (at most 2, 0 output changes) over 2 surfaces
- **Top 300 px at rest** ([shot](2026-11-28-cel-d-018-wallpaper-spotlight/rest-top.jpg)): the picture's light grey-brown strip is at full brightness behind both capsules and the clock; below the bar the picture darkens steadily, and by about y 200 it has reached its dimmest, with no step
- **Menu open** ([shot](2026-11-28-cel-d-018-wallpaper-spotlight/menu-top.jpg)): the volume window hangs under the end capsule over the dimmed part; same wallpaper as at rest, nothing else changes
- **Full output** ([shot](2026-11-28-cel-d-018-wallpaper-spotlight/full-output.jpg)): the yellow and black sculpture on a grey-brown ground, light at the top edge and darker down the screen, the whole lower part evenly dim
- **Seam at scale 1.25 and 1.333** ([1.25](2026-11-28-cel-d-018-wallpaper-spotlight/scale-1-25-seam-zoom.jpg), [1.333](2026-11-28-cel-d-018-wallpaper-spotlight/scale-1-333-seam-zoom.jpg), y 190 to 210 logical, doubled): no hairline or step. At 1.333 the seam sits at device row 266.6, not whole. Row means (800 px wide, left side) fall by about 0.2 per device row up to y 200 and are flat after it (0.04 or less at the junction, inside the picture's own noise)
- **After the one-fill change (`5307fc3a`):** all shots were retaken with the binary built from that commit, replacing the first set (two fills). Mean brightness at scale 1.5, first set against retaken: y 80 117.31 and 117.31, y 300 86.59 and 86.72, near the bottom 57.69 and 57.80; below y 200 the picture is as dim as before, evenly, down to the bottom edge

## Limits

- The nest's picture is the author's real wallpaper, but the nest has one output; other outputs, a window under the bar and the lock screen were not looked at (`VAL-CEL-D-018`).
- Only scales 1.5, 1.25 and 1.333 were seen (1.75 was not used: 200 times 1.75 is whole). The seam was judged on one frame by eye and by row means, not by a pixel diff against a veil-free frame.
- The brightness of the strip at the top of the picture depends on the picture: with a dark top edge the spotlight is hardly visible.
- The draw order in `frame()` (picture, then veil) has no test; the nest shots are its evidence.
- The look in the author's real session is left to `VAL-CEL-D-018`.
- **Final review (Opus, high, `/code-review`), 2 rounds.** Round 1: 0 Critical, 1 Important, 3 Minor, 3 Nit. The Important (two antialiased fills met at y 200, so a fractional scale such as 1.333 could show a lighter row) and the Minor and Nit items were fixed in `5307fc3a`: the veil is one fill whose gradient clamps below y 200, and it spans the picture's own extent. Round 2: no findings but a line-length nit. The shots were retaken after the fix, at scales 1.5, 1.25 and 1.333.
- **Left open:** no test draws a real GL frame or pins the draw order; no shot with a window under the bar, on a second output or of the lock screen. `VAL-CEL-D-018` covers them.

## Landing

- **Base revision:** `2632647a8910a14167eb542af8bd6b79795ebac2`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,wallpaper only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:80abfc467f92d58f5ec3b840b2d33c6b9713d6e7df26ba044636c26bd07d46a4, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
