# What three monitors broke that one never could

- **Date:** 2026-08-17
- **Scope:** Celestina unit `LIVE-1-A`
- **Artifact:** `blurreach.h` and its call sites, `PanelAttachmentLease` and
  `SessionStatus.qml`
- **Environment:** the author's real three-output session (DP-1 4K at
  compositor scale 1.5, DP-2 and HDMI-A-1 at 1080p) on the patched niri built
  from the session's own release commit `8ed0da4`; measured with `grim`
  per output, `gpu-screen-recorder` for the moving cases, the DIAG-1 journal,
  and temporary tracing compiled into a separate compositor binary
- **Plan:** [live session repairs](../plans/active/2026-08-17-live-session-repairs.md)
- **Validation:** `VAL-R8`

## Procedure

Five defects the author reported from live use were each driven to a
mechanism before anything was changed: a crash that ended every migration
attempt, a membrane that appeared on one monitor only, a session that looked
oversaturated, Wi-Fi and Bluetooth missing from the bar, and two blur
materials that looked identical. Each repair was then verified against the
same live session, and two of them against instrumentation rather than
inference.

## Result

### The crash: a request to a surface Qt had already destroyed

`ext-background-effect-v1` answers `set_blur_region` on a destroyed
`wl_surface` with a fatal protocol error, and the client dies on every output
at once. The measured error was
`ext_background_effect_surface_v1#79: error 0: wl_surface was destroyed`.

The shell sent exactly that. Qt Wayland destroys the `wl_surface` when a
window hides but keeps its `QPlatformWindow`, so `QPointer` and `handle()`
both stay non-null across precisely the gap that matters — the first repair
guarded on `handle()` and the crash survived it. Visibility is what tracks the
surface. `blurreach.h` now owns the decision for all nine call sites.

The two directions are deliberately asymmetric. Withdrawing requires
visibility, because a hidden window has nothing to withdraw from. Arming does
not, because `DenseGlassAggregator` arms a companion's region *before* showing
it — KWindowSystem caches the region and applies it on the next expose — and
showing a companion first hands the compositor a mapped surface with no
region, whose per-namespace rule then saturates the whole output. Gating both
would have traded the crash for that defect.

The shell then ran for over four minutes of opening and closing menus without
a protocol error, where before it died in under one.

### The membrane: a lease released on every output but the primary

`PanelAttachmentLease` re-checked on every refresh that the panel and the menu
surface reported the same screen, and released permanently when they did not.
On Wayland a client is not told which output holds its surface until
`wl_surface.enter` arrives, and Qt answers `screen()` with the *primary*
screen until then — while the refresh runs on a zero-delay timer, before that
event.

The author's primary screen is `HDMI-A-1`, so on DP-1 and DP-2 the panel and
its menu looked like different outputs and the attachment was cancelled every
time. A single-output nest cannot produce the mismatch, which is why months of
nested verification never saw it. The lease now records the output once at
acquisition — by construction the panel's own — and releases only when the
panel really moves.

The author confirmed the membrane on all three monitors.

### Wi-Fi and Bluetooth: a visibility cycle, not missing data

Both providers were publishing throughout; the control centre displayed
`Tonys 1` and `S25 Ultra de Antonio` at the same moment the bar showed
neither. Temporary tracing settled it:

    netChanged def= true    linkVisible= false
    btChanged  adapter= on  radioVisible= false

Both conditions were true and both indicators were invisible, because a parent
hides its children in QML: `PanelCluster.visible` was driven by
`hasVisibleIndicator`, which asked `link.visible || radio.visible` — a
question whose answer the cluster already controlled. The group hid itself
because its children looked invisible, and the children were invisible because
the group was hidden.

`Panel.qml` records the tray meeting this same cycle ("four valid items and no
pixels") and answering it by keying visibility off the model. The connectivity
group stayed keyed off rendering and repeated it. It now derives `linkPresent`
and `radioPresent` from the readings.

### The blur: the veil was never too weak, the session profile was missing

The author's report was that both materials looked the same and that the veil
should be much softer. Two wrong turns are worth recording because they cost
the most time.

First, saturation was measured as a property of the composed frame, and the
wallpaper measured identical to its source file (ratios 0.90–1.04). That was
true and irrelevant: `zwlr-gamma-control` applies night light in the hardware
LUT, *after* composition, so a screenshot cannot see it. The author sees a
layer no capture contains.

Second, the compositor patch was declared inert. `passes` and `offset`
produced no change at 4/6, at 8/30, at 1/1, with and without `blur true`. A
first nest run appeared to confirm it — every traced surface reported
`rule.passes=None`. That run never opened a menu, so no dense-glass companion
ever existed and the only surfaces traced were panels, whose rule legitimately
names no strength. Repeating it with a screenshot confirming the control
centre was open gave the opposite answer, end to end:

    update: rule.passes=Some(4) rule.offset=Some(6.0)
    render: blur_options=Some(BlurOptions { passes: 4, offset: 6.0 })
    shader: passes=4 offset=6

The patch works. The real cause was that the live configuration carried no
global `blur {}` block, so the veil fell back to niri's default —
`passes 3, offset 3, saturation 1.5` — which is nearly as strong as the dense
profile and more saturated than the wallpaper. The two materials were
identical because the veil was too strong, exactly as the author said, and no
amount of strengthening the dense cards could express a difference. The
session now carries the profile the material was tuned against in the nest:
`passes 2, offset 2, noise 0.01, saturation 0.9`.

## Limits

None of these repairs is confirmed by a full day of use; they are confirmed by
the specific case each one addresses. The crash is proved only by four minutes
without recurrence, which is strong for a defect that fired within one minute
but is not proof. `VAL-R8` remains the standing question.

The blur profile is a design value, not a proven one: the numbers come from
the nest's reference and the author has not yet said the two materials read
correctly on all three monitors. `passes` and `offset` are accepted only by
the patched compositor; returning to the distribution's binary requires
removing them.
