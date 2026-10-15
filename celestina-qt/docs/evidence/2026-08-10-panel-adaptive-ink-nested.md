# PANEL-1-B adaptive wallpaper ink nested comparison

> Historical prototype evidence: the author later rejected adaptive foreground
> selection. The cumulative `PANEL-1-B` delivery removes this pipeline and keeps
> only fixed light/white shell ink; this record preserves the comparison that
> led to that decision.

- **Date:** 2026-08-10
- **Scope:** bounded per-output wallpaper contrast analysis, exact decoded-source
  correlation, and shell-local light/dark ink propagation
- **Artifact:** final rebuilt `PANEL-1-B` Celestina build-tree candidate
- **Environment:** the already-running nested Niri development session,
  `wayland-2`, output `winit`, 1896 by 998 logical pixels at scale 1
- **Authority boundary:** no production deployment and no activation or
  replacement of the host Noctalia session

## Procedure

### Implemented boundary

The aggregate provider decodes the selected PNG, JPEG, WebP or AVIF away from
the Qt thread under file-size, image-dimension, allocation and 256-by-256 sample
bounds. It opens candidates non-blocking and refuses non-regular files, retains
only the last failed fingerprint per path, and uses a monotonic output-inventory
generation with a predicate wait. An analysis is neither published nor admitted
to hysteresis if its output geometry changed while decoding. The pure shell core
applies the centred `PreserveAspectCrop`, composites alpha over the established
fallback matte, evaluates both semantic ink candidates with WCAG relative
luminance and contrast, and retains a previous choice only through bounded
hysteresis. The additive `wallpaper-appearance` provider publishes one flat
scalar row per output.

The Qt host validates the row's output, source, file revision, inventory
generation and geometry. Those values are part of the internal QML image request
and are accepted only after that exact request reaches `Image.Ready`; replacing
bytes at the same path therefore reloads the image before its new ink can appear.
Loading, stale, malformed and no-blur paths retain light ink over the dark
fallback.

`BackdropInk` derives a surface-local palette without changing the process-wide
`CelestinaTheme` scheme. Every neutral or semantic foreground it exposes uses
the exact measured `#f7f8fc` or `#050608` candidate; hierarchy no longer weakens
text through an unmeasured muted colour. An uncertain sample retains its measured
winner at full strength and only biases the already-low-opacity local veil
toward the opposite candidate, without adding a shadow or increasing opacity or
blur. The panel, the three real Qt menus, the workspace map, control centre,
clipboard, notification centre, session menu and launcher all receive the
output-local tone and uncertainty. Toasts, OSD and the output chooser remain
outside this slice.

## Result

### Automated evidence

The following completed against the same checkout:

- `bash scripts/check-architecture-contract.sh`: passed the sealed colour,
  contrast, QML visual and architecture contracts.
- `bash celestina/scripts/qmllint-production.sh`: passed without warnings.
- `cargo test --manifest-path celestina-rs/Cargo.toml --locked -p
  celestina-shell-core`: 325 passed, including 11 wallpaper contrast tests.
- `cargo test --manifest-path celestina/Cargo.toml --bin
  celestina-provider-adapter`: 66 passed, including bounded output geometry,
  stale-generation rejection, non-regular-file refusal, failure-cache and
  compatibility tests.
- `cargo clippy --manifest-path celestina/Cargo.toml --bin
  celestina-provider-adapter -- -D warnings`: passed.
- The application and the five affected C++/QML test targets compiled and
  linked. Focused CTest passed 5 of 5: shell service, surface manager, overlay
  contract, indicator menu and the complete QuickTest target. The surface test
  also replaced an image at the same path and observed the second revision
  reach `Image.Ready`.
- The broader CTest run passed 16 tests in the restricted sandbox. Its one
  private-D-Bus tray test could not start there; running that exact test outside
  the sandbox passed 4 of 4. This was an execution-environment restriction, not
  an adaptive-ink failure.

### Nested compositor comparison

`celestina/scripts/dev-session.sh --restart` rebuilt and restarted Celestina in
the recorded nested session. It stopped only the previous nested Celestina host,
then started the final rebuilt helper and host. Noctalia remained alive on the
outer session. The successful run mapped the panel on `winit`, armed compositor
blur for its finite regions and logged no wallpaper-analysis, QML construction
or required-property error.

The comparison used these exact files:

- dark current wallpaper `winit.png`:
  `708e8452946ff23ab8600f0b68cdab4818d5d53a4d4e1560fdab3097e040309f`
- bright previous wallpaper `default.jpg`:
  `0f084106a1e5c40f6b3ac1313b064fae1503a7af5ab0160658b97885dfa2c79c`

With the final restarted bytes, the dark wallpaper made the panel and open
control centre use light ink. A temporary `winit.jpg` symlink selected the
bright previous wallpaper without a host restart; after the five-second
provider interval, both the panel and the already-open control centre changed to
dark ink. Removing that exact temporary link restored `winit.png` and light ink
after the next interval. The temporary link is absent, the control centre is
left open for inspection and the nested session is left on the requested dark
wallpaper.

This proves the live one-output transition and exact-source propagation in the
nested compositor.

## Limits

It is not author acceptance of the full all-menu matrix, a
scale-2 result, a two-output result, or a guarantee for a wallpaper region that
contains simultaneous near-black and near-white detail. The pure result marks
that last case as uncertain because no single text colour can provide 4.5:1 at
both extremes.
