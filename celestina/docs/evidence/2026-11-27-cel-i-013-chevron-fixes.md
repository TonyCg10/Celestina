# Evidence: the tray chevron's plate and where its window opens (CEL-I-013)

- **Date:** 2026-11-27
- **Scope:** `CEL-I-013`: `celestina/src/pieces/tray.rs`, `celestina/src/island/sheets.rs`, `celestina/src/island/tests/tray_overflow.rs`, the ledger row and `VAL-CEL-I-013`
- **Environment:** worktree `celestina-CEL-I-013` from `main` at `4997e035` (celestina 1.29.0); the nested look ran on the nest's own bus; debug build in a private target directory
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** none new. The chevron fix restores the look approved on 2026-11-26 (Windows style, list G2, `/home/toni/CODIGO/previews/2026-11-26-cel-i-010-012.html`): an 18 px glyph in the middle of a 32 px box that turns about its own centre. The window's place is the author's request of 2026-11-26, after he tried 1.29.0 live ("tiene problema el hover del chevron y el menu se aleja mucho del icono"): centred under the chevron, as the approved preview animated it, instead of lined up with the right capsule's right edge.

## Cause

- **Plate:** `centred()` sets only `align_items`. In the chevron's row the glyph sat at the box's left edge, 7 px off its centre, so the hover and open plate looked lopsided and the glyph turned about a point to the left of the plate's centre.
- **Window:** `live_anchor` lined every capsule window up with its zone's edge (`CEL-D-009`). The chevron sits at the tray's left end, so with pieces after the tray the window opened far to its right (about 700 px on the author's bar).

## Change

- The chevron's row also centres on its main axis (`justify_content` centre).
- For the chevron, `live_anchor` takes the chevron's own rect as the window's anchor with `Edge::Centre`. `geometry::window` keeps the 16 px margins from the surface's sides and the top at 56. Every other window keeps its zone's edge.

## Procedure

```sh
cd celestina
cargo fmt --all -- --check                                       # ok
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo test --workspace --locked          # 2733 passed, 0 failed, 2 ignored
```

The counts are the sum of every `test result` line. The two new tests failed before the change (RED) and pass after it:

- `the_chevron_glyph_sits_in_the_centre_of_its_box`;
- `overflow_window_centres_under_the_chevron` (replaces `overflow_window_aligns_to_the_capsule_right_edge`).

The nest used CEL-I-012's guarded kit: `env -i`, `--own-bus`, fake `wpctl` and `pactl`, nested niri at scale 1.5 on a 2560 by 1440 logical output, the nest lock, and three fake `StatusNotifierItem`s, one of them hidden and asking for attention. The end zone held `sysmon`, `tray`, `notifications`, `clock` and `session`, so the chevron sits away from the capsule's right edge. The shots are the bar's right part, 1300 by 300 physical pixels.

## Result

- [chevron-zoom.png](./2026-11-27-cel-i-013-chevron-fixes/chevron-zoom.png): the chevron at rest, on hover, mid-turn and open, zoomed. The glyph sits in the middle of the plate in each, and mid-turn it turns about that middle.
- [chevron-hover.png](./2026-11-27-cel-i-013-chevron-fixes/chevron-hover.png): the hover plate, the size of a tray icon's.
- [window-birth.png](./2026-11-27-cel-i-013-chevron-fixes/window-birth.png) and [chevron-turning.png](./2026-11-27-cel-i-013-chevron-fixes/chevron-turning.png): the window grows out of the chevron, under it.
- [window-open.png](./2026-11-27-cel-i-013-chevron-fixes/window-open.png): the window spans x 2196 to 2435 (logical), centred on the chevron at x 2316; the capsule's right edge is at 2543.
- [window-closing.png](./2026-11-27-cel-i-013-chevron-fixes/window-closing.png): the window blurs out towards the chevron.

## Limits

- Fake items only, one output; the author's real bar and two outputs are `VAL-CEL-I-013`.
- Another session's nest ran during the first of the three nest runs (a rest shot, not kept). The shots kept come from runs with no other nest.
- The design's §4.3 and §5 still say the window lines up with the capsule's edge. A documents unit of `TRK-1` amends them; this record and the code follow the author's request.

## Follow-up

- Amend the design §4.3 and §5 through a `TRK-1` documents unit.

## Landing

- **Base revision:** `4997e0356db828e0af96f7467eb7efbe97d3c1a5`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:1689893f256759b56c721e380eb1e3126e5697b470c049b18652d4bbbcbc01f3, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
