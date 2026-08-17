# Why the real session is not the nest: the first live migration, investigated

- **Date:** 2026-08-17
- **Scope:** the whole shell against a real three-output session — the crash
  that ended the first live migration attempt, the per-monitor divergence, the
  global saturation shift, and the night-light limitation
- **Artifact:** celestina 0.30.0 as deployed, `/usr/bin/niri` 26.04 (8ed0da4),
  and the nest compositor at `~/.local/lib/celestina/niri`
- **Environment:** the author's real session for the observed failure (three
  outputs: DP-1 4K at scale 1.5, DP-2 and HDMI-A-1 at 1080p); static reading,
  the DIAG-1 journal of the crashed run, both vendored smithay revisions, the
  niri source tree, and upstream trackers for everything else. Nothing was
  launched on the real session during this investigation.
- **Validation:** `VAL-R8`

## Procedure

The author migrated the live session to Celestina for the first time. The
shell ran correctly on all three outputs for ~55 seconds and died with
`error in client communication` from niri and a fatal Wayland protocol error
on the client. The author additionally reported, from live use: a global
colour-saturation shift while Celestina runs; per-monitor randomness (no blur
at all on one output, only some menus animating on another, the membrane never
connecting on a third); and that night light's temperature is not adjustable.

The DIAG-1 journal of the crashed run was read (its last records are a
`ctx.menu` of kind `capture` opened inside the workspace-map overlay, then one
more `blur.armed` on that overlay). Both vendored smithay revisions'
`background_effect` modules were diffed for their protocol-error conditions.
The stock and nest compositors were fingerprinted. A static audit of the shell
and targeted upstream research (niri and kwindowsystem trackers, protocol
spec) were run in parallel.

## Result

### The nest has been a different compositor all along

The nest build script asks for the installed niri's version tag but falls back
silently to the default branch when the tag fetch fails. The nest compositor
is niri commit `6062844`, dated **2026-08-14** — `main` from three days ago —
while the real session runs the **26.04 release from April** (`8ed0da4`).
Every visual verification of the glass, the membrane and the blur happened
against a compositor roughly five months newer than the one the session runs,
one that carries post-release blur fixes the release does not
(niri #4429, closed 2026-08-11; the #3660 teardown fix class; parts of #4395).
This single fact explains most of "it works in the nest and misbehaves live".

### The crash: a teardown-ordering protocol error, with two client-side
### suspects kept honest

`ext-background-effect-v1` has exactly two protocol errors, identical in both
vendored smithay revisions: a second effect object on one surface, and any
request after the `wl_surface` died. Upstream, niri #3660 documents precisely
this fingerprint killing Dolphin — a Qt + KWindowSystem client, the same stack
as Celestina: KWindowSystem tears the effect object down via `deleteLater()`,
so its destroy lands **after** Qt destroyed the `wl_surface`, and 26.04-era
niri answers with a fatal error. Celestina widens that window from its own
side: the falling membrane re-arms its blur region on every animation frame
(`panelblurcontroller.cpp`), and the withdraw path fires "unconditionally,
exposed or not" — either can hand KWindowSystem work that lands on a surface
Qt has already destroyed while the overlay animates closed.

The static audit adds two independent crash-class defects that the protocol
error could be masking and that stand on their own:

- `AppIconProvider` resolves `QIcon::fromTheme(...).pixmap(...)` on Qt Quick's
  image-loader thread. `QIconLoader` and `QPixmap` are GUI-thread machinery,
  and the same process uses them from the GUI thread continuously (tray). The
  two failed lookups logged seconds before death (`com.anthropic.Claude`,
  `Chatgpt`) are the worst case — a miss walks every theme directory,
  maximizing the race window. Corruption from a cross-thread race can surface
  as a malformed Wayland request blamed on whatever committed next.
- The quiet-surface `followSize` path forwards the content window's size to
  `setDesiredSize` unclamped. A transient zero height (last toast retiring) on
  a single-edge-anchored layer surface is a fatal `zwlr_layer_surface_v1`
  protocol error on any wlr-layer-shell compositor.

### The saturation shift is real and has a mechanism

niri's default blur saturation is **1.5** (`niri-config`), and the author's
live configuration carries no profile that lowers it — the tuned profile
exists only in the nest's config. Worse, `R8-P-I` already measured the
underlying rule: a mapped surface with a matching `background-effect`
layer-rule and *no* effective region carries the effect over its whole
geometry. The fix unmapped resting companions, but the aggregator still maps a
companion **before** its first region is armed — at least one whole-output
saturation frame per dark section — and every one of these surfaces exists
per output, so the flash pattern differs per monitor.

### The per-monitor randomness has four compounding mechanisms

1. **Documented compositor behaviour at 26.04:** non-xray effects — which the
   live config selects for every Celestina namespace via `xray false` — are
   experimental in the release, and the effect *disappears during open/close
   animations and drags*. Fixed after the release; the nest has those fixes.
2. **Per-output frame pacing:** niri times animations against each output's
   presentation clock and throttles surfaces it considers not visible to
   ~1 Hz; a menu on the throttled output reads as "does not animate".
3. **Per-output gating in the shell:** opener/anchor rectangles are divided by
   a per-screen shell scale derived from EDID; an output whose scale resolves
   unexpectedly fails the membrane's preconditions, which also keeps its glass
   regions unpublished — so that output gets neither the membrane nor blur.
   One root cause, two symptoms, per output.
4. **Fail-soft companions:** a companion that fails to map is silently
   skipped, leaving an output with fewer blur samples — visibly weaker
   material on exactly one monitor — and companions are keyed by raw
   `QScreen*` with no unplug handling.

### Night light

The temperature is a constant — 2700 K in
`celestina-shell-core::nightlight` — applied per output through gamma
control. Not configurable anywhere in the pipeline; making it adjustable from
the control centre is a feature, not a repair.

## Limits

The exact protocol error name of the live crash was never captured — niri
logs only `error in client communication` at default verbosity, and the
client sees a generic fatal error. The teardown-ordering diagnosis is the
best fit (upstream-documented, same client stack, same fingerprint) but is
not proven against this specific crash; the two audit findings are real
defects regardless of which one fired. Whether the release-versus-main gap
alone accounts for the whole saturation shift was not measured on the live
session — the R8-P-I measurement method exists for that and was not re-run.
Nothing here was fixed; this record authorizes no code change by itself.
