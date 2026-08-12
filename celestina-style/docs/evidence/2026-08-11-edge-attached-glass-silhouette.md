# STYLE-G7-J prototype — continuous veil and elastic membrane material

- **Date:** 2026-08-11
- **Scope:** CelestinaStyle unit `STYLE-G7-J`
- **Artifact:** CelestinaStyle 1.4.0 milestone prototype
- **Environment:** Linux, Qt 6.9 compiled-module floor, CMake Release build and
  offscreen Qt platform for automated construction and smoke
- **Plan:** [shared reading controls ledger](../plans/active/2026-08-04-shared-reading-controls.md)
- **Consumer validation:** Celestina `PANEL-1-I` and `VAL-PANEL-1`

## Procedure

The current composition uses the existing `ContextualVeil` role as one
continuous, marginless ExternalBackdrop material behind the complete panel.
Celestina owns that edge-to-edge geometry and its single panel compositor
region. The inset panel capsules return to ordinary rounded `ContentSurface`
materials: they request no compositor region of their own and keep the
historical rounded `GlassSurface` default. `ContextualVeil` attenuates only tint
and noise and suppresses outline and lit-edge layers for rounded and shaped
paths, leaving neither an apparent border nor an edge halo.

`GlassSurface.silhouettePath` and its optional open edge-stroke path remain an
opt-in material facility for the contextual membrane. The same generic
renderer accepts arbitrary vector fills while `silhouetteEdgePath` can omit the
physical attachment seam. The `ContextualVeil` role suppresses both shaped
edge layers; an empty silhouette remains pixel-compatible with existing
rounded consumers.
Panel capsules and dense content cards never opt into this path for attachment;
they remain ordinary rounded `ContentSurface` instances throughout.

Style retains only the demonstrated proportional vertical-travel tokens: ratio
`0.06`, clamped to 20..36 pixels. The droplet membrane — its narrow
icon-proportional mouth clinging to the bar seam with a meniscus, its
glyph-centred and flat-span-clamped placement, its tension-thinned hanging
neck, its tangent landing on the body's flat top edge, the painted path,
sampled finite polygon, live anchor tracking, opener
placement and lifecycle — all depend on shell geometry and therefore remain
owned by Celestina. The tokened state that preserves only the opener's ordinary
hover feedback is also shell-local and changes no Style API. There is no
reusable mouth, neck, meniscus, glyph-target, curve-control or opener-state
token in the shared theme. The membrane uses `ContextualVeil` alone; there is
no dense bridge or cross-window material transition.

The focused construction coverage must therefore prove five independent
contracts: the continuous veil paints tint/noise but no rounded or shaped
outline/lit edge, an ordinary rounded ContentSurface needs no shaped path, the
vertical-travel metric matches every real-width row and both clamps, attachment
does not mutate a rounded ContentSurface, and a shaped ExternalBackdrop
membrane enables neither in-scene capture, dense material nor a rectangular
elevation shadow.

## Result

### Droplet current iteration

On 2026-08-11 the author rejected the fluid body-proportional-waist revision
live as a strange hourglass and asked for a soft drop falling out of the bar.
The current checkout keeps the silhouette facility only on the
`ContextualVeil` membrane, now a droplet: its sole seam contact is one narrow
icon-proportional mouth centred on the clicked glyph, clinging to the bar
with a meniscus, narrowing to a hanging neck that shell tension only thins
and swelling until it lands tangent on the body's flat top edge inside its
ordinary rounded corners. Every rounded `ContentSurface` remains
unchanged, with no dense bridge. The neck width, curve construction and
opener's persistent ordinary hover feedback remain shell-local effects and do
not extend `GlassSurface`, `CelestinaTheme` or any other Style API. This is the
active compatibility contract above.

Canonical Style verification for this droplet revision
passes production-common 29/29, the architecture, contrast and QML visual
guards, `qmllint` with only the pre-existing `CelestinaLineGutter` warnings,
CTest 1/1 and the eight-second compiled-module smoke. Registered Celestina
completion passes the full Rust suites, CTest 17/17 and its eight-second
release smoke, then deploys and reports the current verified artifacts without
activating or replacing a live session. Perceptual author validation remains
pending.

### Superseded prototype evidence

The immediately preceding fluid body-proportional-waist revision kept both
body-wide edges, scaled its waist from `0.78` to `0.64` of body width and
joined its cubic halves with C2 continuity. Its canonical Style verification
and registered Celestina completion passed the same recorded workflows, but
the author rejected its live hourglass read on 2026-08-11. Those results
verify only that superseded geometry; they do not verify the current droplet
mouth, meniscus, neck or tangent landing.

The body-wide icon-scaled revision before it used the complete
contextual body at both endpoints but sized its narrow waist from the clicked
glyph. Its Celestina focal QuickTest passed 210/210, canonical Style
verification passed 29 production-common fixtures, CTest 1/1 and the
compiled-module smoke, and registered Celestina completion passed the complete
Rust suites, QML lint, CTest 17/17 and its release smoke before deploying the
verified bundle without session activation. Those results verify only that
superseded icon-scaled geometry; they do not verify the current
body-proportional waist or C2 join.

The immediately preceding glyph-mouth revision kept the rounded capsules and
veil-only membrane but made the membrane's upper span equal the exact clicked
glyph. It passed focused consumer CTest 4/4 and QuickTest 208/208. Canonical
Style verification passed 29 production-common fixtures, semantic guards, QML
lint, CTest 1/1 and the eight-second compiled-module smoke; registered Celestina
completion passed CTest 17/17 and its release smoke, then deployed that verified
bundle without session activation. Those results are explicitly superseded and
do not verify either body-wide revision or persistent opener feedback.

The earlier whole-capsule consumer revision used the complete owning capsule
as its mouth, opened that capsule and painted a dense-to-veil transition. It
passed:

```sh
bash scripts/check-architecture-contract.sh
bash celestina-style/scripts/build-production.sh
bash celestina-style/scripts/verify-production.sh
ctest --test-dir celestina/build --output-on-failure \
  -R '^(celestina-surface-manager|celestina-overlay-contract|celestina-indicator-menu|celestina-output-chooser)$'
bash celestina/scripts/complete-production.sh
```

The architecture contract passed for that revision. The canonical Style
artifact built and verified, including 29 production-common fixtures, semantic
guards, QML lint, CTest 1/1 and its eight-second compiled-module smoke. The
focused Celestina consumer selection passed 4/4 and its QuickTest runner
reported 208/208 cases,
including the open active capsule, full-span bounded elastic membrane and live
owner geometry/lifetime regressions. Those results do not verify either the
later glyph-mouth correction or the current droplet, immutable-surface and
veil-only membrane contract. The
registered Celestina completion then passed the complete Rust suites, QML lint,
CTest 17/17 and its release smoke, deployed the verified bundle to the normal
test prefix and reported every installed artifact current without activating a
session. An earlier sandboxed completion stopped only at the tray-watcher
fixture's private D-Bus socket; the registered runs with `/tmp` socket access
pass it. `STYLE-G7-J` remains `active`; no immutable inventory or commit
exists.

The preceding continuous-veil revision retained the outline/lit-edge recipe and
used one fixed connector anatomy. It passed canonical Style verification and
Celestina completion, including CTest 17/17, and deployed its verified bundle
without activation. Those results do not verify the current layer suppression
or the dynamic droplet membrane.

Before that, the top-edge droplet experiment passed its focused GlassSurface
fixture 7/7, 29 production-common tests, the architecture/style/contrast/visual
guards, `all_qmllint`, CTest 1/1, the eight-second gallery smoke and its manifest
check. Celestina also completed against those bytes. Those results remain useful
compatibility evidence for the vector renderer, but they do not verify the
current body-wide-edge, body-proportional-waist, C2-joined bar-bottom membrane
composition. The active-panel-capsule part of that experiment is not part of
the corrected contract.

## Limits

No live surface was activated or replaced for the current iteration. Automated
construction cannot establish perceived veil density, the absence of a
compositor-scale edge halo, real blur, membrane tension, seam removal or
antialiasing. The complete panel
and contextual-surface comparison remains author-owned under Celestina's
`VAL-PANEL-1` in a nested Niri session.
