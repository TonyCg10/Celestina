# A wallpaper change passed through a frame of bare canvas

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-J`
- **Artifact:** Celestina 0.29.7, `qml/Wallpaper.qml`
- **Environment:** the nested reference session, recorded at 60 fps with
  `gpu-screen-recorder` while the author switched wallpapers from the menu
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The author reported the background still flickering after the saturation
ghost was fixed. A 60 fps recording of their own wallpaper switching was
decomposed: frame f0049 shows the old image, f0050 loses 25 luminance points
in one frame — the bare canvas — and the next image fades in from that over
the following six frames.

## Result

Two behaviours compounded. The loader hides the moment the requested
identity changes — `showingImage` is deliberately strict so a stale
photograph is never presented as a current one — and a QML `Image` drops its
old texture the moment its `source` does. Between them, every wallpaper
switch showed the window's own fill until the new file finished decoding.

A second `Image` now sits under the loader and holds the last file the
loader finished showing. Its source is only ever assigned on the loader's
`Ready`, so it can never resurrect a request that failed or was withdrawn:
an undecodable file still falls through to the deliberate fallback, and an
emptied source still clears to it. What changed is only the transition — the
person keeps looking at the wallpaper they had until the one they chose is
actually ready.

## Limits

The verification recording was made before the fix; after it, the author's
own switching is the check, because the nest's output records black from
outside while that monitor is dark and `grim` is too slow to catch a single
frame. The suite passes at 23/23. Memory holds two decoded wallpapers per
output instead of one, which is the price of never showing the canvas.
